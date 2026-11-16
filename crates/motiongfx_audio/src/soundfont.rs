use crate::engine::{BLOCK, SoundFontVoice, Source};
use crate::{Instrument, LoadError, Pitch};
use std::path::Path;
use std::sync::Arc;

/// A SoundFont (`.sf2`) file, loaded and ready to play. Each preset in
/// bank 0 is an [`Instrument`]: [`program`](Self::program) picks one.
/// Cheap to clone; clones share the file's data.
///
/// # Examples
///
/// ```no_run
/// use motiongfx_audio::SoundFont;
/// # fn main() -> Result<(), motiongfx_audio::LoadError> {
/// let piano = SoundFont::load("assets/piano.sf2")?.program(0);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SoundFont(Arc<rustysynth::SoundFont>);

impl SoundFont {
    /// Read and parse the SoundFont at `path`, on the calling thread.
    ///
    /// # Errors
    ///
    /// If the file can't be read or isn't a valid SoundFont.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, LoadError> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)
            .map_err(|error| LoadError::io(path, error))?;
        let font = rustysynth::SoundFont::new(
            &mut std::io::BufReader::new(file),
        )
        .map_err(|error| LoadError::soundfont(path, error))?;
        Ok(Self(Arc::new(font)))
    }

    /// The instrument for preset `program` in bank 0 (General MIDI
    /// numbering from 0: 0 is usually a piano).
    ///
    /// # Panics
    ///
    /// At the caller's line, if the SoundFont has no such preset.
    #[track_caller]
    pub fn program(&self, program: u8) -> Instrument {
        let found = self.0.get_presets().iter().any(|preset| {
            preset.get_bank_number() == 0
                && preset.get_patch_number() == i32::from(program)
        });
        assert!(
            found,
            "this SoundFont has no preset for program {program} in bank 0"
        );
        Instrument::soundfont(self.0.clone(), program)
    }
}

impl std::fmt::Debug for SoundFont {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("SoundFont")
            .field("presets", &self.0.get_presets().len())
            .finish()
    }
}

/// A synthesizer playing one note, so that note's release can't affect
/// any other, even one of the same key. `None` if rustysynth can't run at
/// `rate` (it supports 16 to 192 kHz).
pub(crate) fn source(
    font: &Arc<rustysynth::SoundFont>,
    program: u8,
    pitch: Pitch,
    velocity: f32,
    rate: u32,
) -> Option<Source> {
    let mut settings = rustysynth::SynthesizerSettings::new(
        i32::try_from(rate).ok()?,
    );
    settings.block_size = BLOCK;
    // One note, but a preset may layer several samples on it.
    settings.maximum_polyphony = 8;
    settings.enable_reverb_and_chorus = false;
    let mut synth =
        rustysynth::Synthesizer::new(font, &settings).ok()?;
    const PROGRAM_CHANGE: i32 = 0xC0;
    synth.process_midi_message(
        0,
        PROGRAM_CHANGE,
        i32::from(program),
        0,
    );
    // Rounded and clamped, so the conversions can't truncate.
    let key = pitch.to_midi().round().clamp(0.0, 127.0) as i32;
    let velocity =
        (velocity * 127.0).round().clamp(1.0, 127.0) as i32;
    synth.note_on(0, key, velocity);
    Some(Source::SoundFont(Box::new(SoundFontVoice {
        synth,
        key,
        left: [0.0; BLOCK],
        right: [0.0; BLOCK],
    })))
}
