use crate::{Mixer, Player, Score};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use std::time::Duration;

/// Frames mixed per step inside the device callback; the scratch buffer
/// holds this many stereo frames, so the callback never allocates.
const CHUNK: usize = 1024;

/// Live playback through the default output device. Keep it alive while
/// sound should play, and drive it like a [`Player`].
pub struct AudioOutput {
    player: Player,
    // Sound stops when the stream is dropped.
    _stream: cpal::Stream,
}

impl AudioOutput {
    /// Open the default output device at its preferred sample rate and
    /// start a silent stream, ready to play `score`.
    ///
    /// # Errors
    ///
    /// If there is no output device, it can't be configured, or it uses a
    /// sample format other than 32-bit float or 16-bit integer.
    pub fn open(score: Score) -> Result<Self, OutputError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(OutputError(Cause::NoDevice))?;
        let supported = device
            .default_output_config()
            .map_err(OutputError::device)?;
        let config = supported.config();
        let (player, mixer) = Player::new(score, config.sample_rate);
        let stream = match supported.sample_format() {
            SampleFormat::F32 => {
                build::<f32>(&device, &config, mixer)
            }
            SampleFormat::I16 => {
                build::<i16>(&device, &config, mixer)
            }
            format => return Err(OutputError(Cause::Format(format))),
        }
        .map_err(OutputError::device)?;
        stream.play().map_err(OutputError::device)?;
        Ok(Self {
            player,
            _stream: stream,
        })
    }

    /// See [`Player::advance`].
    pub fn advance(&mut self, from: Duration, to: Duration) {
        self.player.advance(from, to);
    }

    /// See [`Player::stop_all`].
    pub fn stop_all(&mut self) {
        self.player.stop_all();
    }

    /// See [`Player::set_speed`].
    ///
    /// # Panics
    ///
    /// If `speed` isn't a positive, finite number.
    #[track_caller]
    pub fn set_speed(&mut self, speed: f64) {
        self.player.set_speed(speed);
    }

    /// See [`Player::set_volume`].
    pub fn set_volume(&mut self, gain: f32) {
        self.player.set_volume(gain);
    }
}

impl std::fmt::Debug for AudioOutput {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("AudioOutput")
            .field("player", &self.player)
            .finish_non_exhaustive()
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut mixer: Mixer,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut stereo = vec![0.0f32; CHUNK * 2];
    device.build_output_stream::<T, _, _>(
        *config,
        move |data: &mut [T], _| {
            for chunk in data.chunks_mut(channels * CHUNK) {
                let frames = chunk.len() / channels;
                let stereo = &mut stereo[..frames * 2];
                mixer.render(stereo);
                for (out, lr) in chunk
                    .chunks_exact_mut(channels)
                    .zip(stereo.as_chunks::<2>().0)
                {
                    if channels == 1 {
                        out[0] =
                            T::from_sample((lr[0] + lr[1]) * 0.5);
                        continue;
                    }
                    out[0] = T::from_sample(lr[0]);
                    out[1] = T::from_sample(lr[1]);
                    for extra in &mut out[2..] {
                        *extra = T::EQUILIBRIUM;
                    }
                }
            }
        },
        // The stream reports problems from its own thread; nothing there
        // can return them, so they are printed.
        |error| eprintln!("audio output error: {error}"),
        None,
    )
}

/// The audio device could not be opened.
#[derive(Debug)]
pub struct OutputError(Cause);

#[derive(Debug)]
enum Cause {
    NoDevice,
    Device(cpal::Error),
    Format(SampleFormat),
}

impl OutputError {
    fn device(error: cpal::Error) -> Self {
        Self(Cause::Device(error))
    }
}

impl std::fmt::Display for OutputError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match &self.0 {
            Cause::NoDevice => f.write_str("no audio output device"),
            Cause::Device(error) => {
                write!(f, "could not open the audio output: {error}")
            }
            Cause::Format(format) => write!(
                f,
                "the audio output uses unsupported sample format {format}"
            ),
        }
    }
}

impl std::error::Error for OutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.0 {
            Cause::Device(error) => Some(error),
            Cause::NoDevice | Cause::Format(_) => None,
        }
    }
}
