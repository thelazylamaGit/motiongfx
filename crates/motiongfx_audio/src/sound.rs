//! What one sound event plays, and how it becomes a voice.

#[cfg(any(feature = "fundsp", feature = "rustysynth"))]
use crate::Pitch;
use crate::engine::{Source, Voice};
use std::sync::Arc;

/// The payload of every sound event: what to play and how loud. Private,
/// so [`Score::new`](crate::Score::new) is the only reader.
#[derive(Clone)]
pub(crate) struct Sound {
    pub(crate) kind: SoundKind,
    /// Velocity for instruments (0 to 1), gain for samples.
    pub(crate) level: f32,
}

#[derive(Clone)]
pub(crate) enum SoundKind {
    #[cfg(feature = "fundsp")]
    Synth(Arc<crate::instrument::Patch>, Pitch),
    #[cfg(feature = "rustysynth")]
    SoundFont {
        font: Arc<rustysynth::SoundFont>,
        program: u8,
        pitch: Pitch,
    },
    Sample(Arc<Frames>),
}

/// Decoded audio: interleaved stereo at `rate` frames per second.
pub(crate) struct Frames {
    pub(crate) samples: Box<[f32]>,
    pub(crate) rate: u32,
}

impl Sound {
    /// A voice for this sound that starts at frame `start` and is released
    /// at frame `release`, at `rate` frames per second. Built on the
    /// calling thread, which allocates, so the audio thread never has to.
    /// `None` if the sound can't play at this rate.
    pub(crate) fn voice(
        &self,
        rate: u32,
        start: u64,
        release: u64,
    ) -> Option<Voice> {
        let (source, gain) = match &self.kind {
            #[cfg(feature = "fundsp")]
            SoundKind::Synth(patch, pitch) => {
                (patch.source(pitch.to_hz(), rate), self.level)
            }
            #[cfg(feature = "rustysynth")]
            SoundKind::SoundFont {
                font,
                program,
                pitch,
            } => (
                crate::soundfont::source(
                    font, *program, *pitch, self.level, rate,
                )?,
                // The velocity already sets the loudness.
                1.0,
            ),
            SoundKind::Sample(frames) => (
                Source::Sample {
                    step: f64::from(frames.rate) / f64::from(rate),
                    frames: frames.clone(),
                    position: 0.0,
                },
                self.level,
            ),
        };
        Some(Voice::new(source, gain, start, release, rate))
    }
}
