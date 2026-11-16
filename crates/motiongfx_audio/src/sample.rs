use crate::sound::{Frames, Sound, SoundKind};
use motiongfx_authoring::{Animation, event};
use std::sync::Arc;
use std::time::Duration;

/// A recorded sound, such as an effect, played as it is. Cheap to clone;
/// clones share the decoded audio.
///
/// Every play is its own voice, so overlapping plays layer instead of
/// restarting each other.
///
/// # Examples
///
/// ```
/// use motiongfx::prelude::ms;
/// use motiongfx_audio::Sample;
/// use motiongfx_authoring::{Animation, flow};
///
/// // A 50 ms click: a short decaying burst.
/// let click = Sample::mono(48_000, (0..2400).map(|i| (1.0 - i as f32 / 2400.0) * 0.5).collect());
/// // Five clicks, each a little louder.
/// let clicks: Animation<()> = flow(ms(200), (0..5).map(|i| click.gain(0.6 + 0.1 * i as f32).play()));
/// ```
#[derive(Clone)]
pub struct Sample(Arc<Frames>);

impl Sample {
    /// A sample from mono `samples` at `sample_rate` frames per second.
    ///
    /// # Panics
    ///
    /// At the caller's line, if `sample_rate` is zero.
    #[track_caller]
    pub fn mono(sample_rate: u32, samples: Vec<f32>) -> Self {
        let stereo = samples.iter().flat_map(|&s| [s, s]).collect();
        Self::from_interleaved(sample_rate, stereo)
    }

    /// A sample from stereo `samples` (left and right interleaved) at
    /// `sample_rate` frames per second.
    ///
    /// # Panics
    ///
    /// At the caller's line, if `sample_rate` is zero or `samples` has an
    /// odd length.
    #[track_caller]
    pub fn stereo(sample_rate: u32, samples: Vec<f32>) -> Self {
        assert!(
            samples.len().is_multiple_of(2),
            "stereo samples alternate left and right, so there must be an even number of them, got {}",
            samples.len()
        );
        Self::from_interleaved(sample_rate, samples)
    }

    #[track_caller]
    fn from_interleaved(rate: u32, samples: Vec<f32>) -> Self {
        assert!(rate > 0, "a sample rate must be positive");
        Self(Arc::new(Frames {
            samples: samples.into_boxed_slice(),
            rate,
        }))
    }

    /// Read and decode the sound file at `path` (mp3, wav, ogg/vorbis or
    /// flac) on the calling thread. Mono files play in both channels;
    /// files with more than two channels keep the first two.
    ///
    /// # Errors
    ///
    /// If the file can't be read, has no audio track, or can't be decoded.
    #[cfg(feature = "symphonia")]
    pub fn load(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, crate::LoadError> {
        let path = path.as_ref();
        let (rate, samples) = crate::decode::decode(path)?;
        Ok(Self::from_interleaved(rate, samples))
    }

    /// How long the sample plays at its own rate.
    pub fn duration(&self) -> Duration {
        let frames = self.0.samples.len() / 2;
        Duration::from_secs_f64(
            frames as f64 / f64::from(self.0.rate),
        )
    }

    /// Plays of this sample scaled by `gain` (1 is unchanged;
    /// [`play`](Self::play) plays at 1). Borrows the sample, so nothing
    /// is copied until a play is made.
    ///
    /// # Panics
    ///
    /// At the caller's line, if `gain` is negative or not finite.
    #[track_caller]
    pub fn gain(&self, gain: f32) -> AtGain<'_> {
        assert!(
            gain.is_finite() && gain >= 0.0,
            "gain must be finite and not negative, got {gain}"
        );
        AtGain { sample: self, gain }
    }

    /// Play the sample. On the timeline it lasts the sample's
    /// [`duration`](Self::duration); `.over(..)` shortens it (fading out
    /// over 5 ms) or lengthens it (silence after the sample ends).
    pub fn play<M: 'static>(&self) -> Animation<M> {
        self.gain(1.0).play()
    }
}

impl std::fmt::Debug for Sample {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Sample")
            .field("rate", &self.0.rate)
            .field("duration", &self.duration())
            .finish()
    }
}

/// A [`Sample`] at a chosen gain, from [`Sample::gain`].
#[derive(Clone, Copy, Debug)]
#[must_use = "make a play with `.play()`"]
pub struct AtGain<'a> {
    sample: &'a Sample,
    gain: f32,
}

impl AtGain<'_> {
    /// Play the sample at this gain; see [`Sample::play`].
    pub fn play<M: 'static>(self) -> Animation<M> {
        let sound = Sound {
            kind: SoundKind::Sample(self.sample.0.clone()),
            level: self.gain,
        };
        event(sound, self.sample.duration())
    }
}
