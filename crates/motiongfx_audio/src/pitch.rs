/// A pitch as a MIDI note number: 60 is middle C, 69 is A4 (440 Hz), and
/// each step is a semitone. Fractions fall between semitones.
///
/// Whole notes convert from `u8`, so scales and intervals are integer
/// arithmetic: `instrument.note(key + 7)` is a fifth above `key`.
///
/// # Examples
///
/// ```
/// use motiongfx_audio::Pitch;
/// assert_eq!(Pitch::from(69).to_hz(), 440.0);
/// assert_eq!(Pitch::hz(880.0).to_midi(), 81.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Pitch(f32);

impl Pitch {
    /// The pitch of MIDI note `note`, which may have a fraction.
    ///
    /// # Panics
    ///
    /// At the caller's line, if `note` isn't finite.
    #[track_caller]
    pub fn midi(note: f32) -> Self {
        assert!(
            note.is_finite(),
            "a pitch must be finite, got {note}"
        );
        Self(note)
    }

    /// The pitch with frequency `hz`.
    ///
    /// # Panics
    ///
    /// At the caller's line, if `hz` isn't finite and positive.
    #[track_caller]
    pub fn hz(hz: f32) -> Self {
        assert!(
            hz.is_finite() && hz > 0.0,
            "a frequency must be finite and positive, got {hz}"
        );
        Self(69.0 + 12.0 * (hz / 440.0).log2())
    }

    /// The frequency in hertz.
    pub fn to_hz(self) -> f32 {
        440.0 * ((self.0 - 69.0) / 12.0).exp2()
    }

    /// The MIDI note number, with any fraction.
    pub fn to_midi(self) -> f32 {
        self.0
    }
}

impl From<u8> for Pitch {
    fn from(note: u8) -> Self {
        Self(f32::from(note))
    }
}
