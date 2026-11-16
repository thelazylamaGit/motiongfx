use crate::Pitch;
use crate::sound::{Sound, SoundKind};
use motiongfx_authoring::{Animation, event};
use std::sync::Arc;
use std::time::Duration;

/// How long a note is held when it doesn't say, with `.over(..)`. The
/// release rings on after this.
pub const NOTE_LENGTH: Duration = Duration::from_millis(250);

/// A pitched sound to play notes on: a synth, or a program of a
/// [`SoundFont`](crate::SoundFont). Cheap to clone; clones share the
/// sound.
///
/// Every note is its own voice, so overlapping notes, even of the same
/// pitch, never cut each other off.
///
/// # Examples
///
/// ```
/// use motiongfx::prelude::ms;
/// use motiongfx_audio::Instrument;
/// use motiongfx_audio::fundsp::prelude32::{adsr_live, sine_hz};
/// use motiongfx_authoring::{Animation, chain};
///
/// let sine = Instrument::synth(|hz| sine_hz(hz) * adsr_live(0.01, 0.1, 0.6, 0.2));
/// // C, E, G, then a soft C an octave up, held for a second.
/// let arpeggio: Animation<()> = chain([
///     sine.note(60),
///     sine.note(64),
///     sine.note(67),
///     sine.velocity(0.5).note(72).over(ms(1000)),
/// ]);
/// ```
#[derive(Clone)]
pub struct Instrument(Kind);

#[derive(Clone)]
enum Kind {
    #[cfg(feature = "fundsp")]
    Synth(Arc<Patch>),
    #[cfg(feature = "rustysynth")]
    SoundFont(Arc<rustysynth::SoundFont>, u8),
}

impl Instrument {
    /// An instrument whose notes are fundsp graphs: `voice(hz)` builds the
    /// graph for one note at `hz`.
    ///
    /// The graph has one input, the gate: 1 while the note is held and 0
    /// after its release, which envelopes such as `adsr_live` read. It
    /// has one output (played in the centre) or two (left and right). A
    /// graph without an input fades out over 5 ms at the release.
    ///
    /// # Panics
    ///
    /// At the caller's line, if the graph `voice(440.0)` builds has more
    /// than one input, or other than one or two outputs.
    #[cfg(feature = "fundsp")]
    #[track_caller]
    pub fn synth<F, U>(voice: F) -> Self
    where
        F: Fn(f32) -> U + Send + Sync + 'static,
        U: fundsp::audiounit::AudioUnit + 'static,
    {
        let probe = voice(440.0);
        let (inputs, outputs) = (probe.inputs(), probe.outputs());
        assert!(
            inputs <= 1,
            "a synth voice takes at most one input (the gate), but this one takes {inputs}"
        );
        assert!(
            outputs == 1 || outputs == 2,
            "a synth voice has one or two outputs, but this one has {outputs}"
        );
        Self(Kind::Synth(Arc::new(Patch {
            build: Box::new(move |hz| Box::new(voice(hz))),
            gated: inputs == 1,
            stereo: outputs == 2,
        })))
    }

    #[cfg(feature = "rustysynth")]
    pub(crate) fn soundfont(
        font: Arc<rustysynth::SoundFont>,
        program: u8,
    ) -> Self {
        Self(Kind::SoundFont(font, program))
    }

    /// Notes played at `velocity` (0 to 1; [`note`](Self::note) plays at
    /// 1). Synths scale their output by it; SoundFonts pass it on as the
    /// MIDI velocity, which can also change the timbre. Borrows the
    /// instrument, so nothing is copied until a note is made.
    ///
    /// # Panics
    ///
    /// At the caller's line, if `velocity` is outside 0 to 1.
    #[track_caller]
    pub fn velocity(&self, velocity: f32) -> AtVelocity<'_> {
        assert!(
            (0.0..=1.0).contains(&velocity),
            "velocity must be between 0 and 1, got {velocity}"
        );
        AtVelocity {
            instrument: self,
            velocity,
        }
    }

    /// A note at `pitch`, held for [`NOTE_LENGTH`] unless `.over(..)` says
    /// otherwise. It lasts that long on the timeline; its release rings
    /// on afterwards without holding anything up.
    pub fn note<M: 'static>(
        &self,
        pitch: impl Into<Pitch>,
    ) -> Animation<M> {
        self.sound(pitch.into(), 1.0)
    }

    fn sound<M: 'static>(
        &self,
        pitch: Pitch,
        level: f32,
    ) -> Animation<M> {
        let kind = match &self.0 {
            #[cfg(feature = "fundsp")]
            Kind::Synth(patch) => {
                SoundKind::Synth(patch.clone(), pitch)
            }
            #[cfg(feature = "rustysynth")]
            Kind::SoundFont(font, program) => SoundKind::SoundFont {
                font: font.clone(),
                program: *program,
                pitch,
            },
        };
        event(Sound { kind, level }, NOTE_LENGTH)
    }
}

impl std::fmt::Debug for Instrument {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match &self.0 {
            #[cfg(feature = "fundsp")]
            Kind::Synth(_) => f.write_str("Instrument(synth)"),
            #[cfg(feature = "rustysynth")]
            Kind::SoundFont(_, program) => {
                write!(f, "Instrument(SoundFont program {program})")
            }
        }
    }
}

/// An [`Instrument`] at a chosen velocity, from
/// [`Instrument::velocity`].
#[derive(Clone, Copy, Debug)]
#[must_use = "make a note with `.note(pitch)`"]
pub struct AtVelocity<'a> {
    instrument: &'a Instrument,
    velocity: f32,
}

impl AtVelocity<'_> {
    /// A note at `pitch` at this velocity; see [`Instrument::note`].
    pub fn note<M: 'static>(
        self,
        pitch: impl Into<Pitch>,
    ) -> Animation<M> {
        self.instrument.sound(pitch.into(), self.velocity)
    }
}

/// Builds a synth's graph for each note.
#[cfg(feature = "fundsp")]
pub(crate) struct Patch {
    build: Box<
        dyn Fn(f32) -> Box<dyn fundsp::audiounit::AudioUnit>
            + Send
            + Sync,
    >,
    gated: bool,
    stereo: bool,
}

#[cfg(feature = "fundsp")]
impl Patch {
    /// A fresh graph for a note at `hz`, ready to play at `rate`.
    pub(crate) fn source(
        &self,
        hz: f32,
        rate: u32,
    ) -> crate::engine::Source {
        let mut unit = (self.build)(hz);
        unit.set_sample_rate(f64::from(rate));
        unit.reset();
        if self.gated {
            // Envelopes such as `adsr_live` start their attack when the
            // gate rises, so show them one frame of a low gate first. Done
            // here, not on the audio thread; the frame isn't heard.
            let mut discard = [0.0; 2];
            let outputs = if self.stereo { 2 } else { 1 };
            unit.tick(&[0.0], &mut discard[..outputs]);
        }
        crate::engine::Source::Synth {
            unit,
            gated: self.gated,
            stereo: self.stereo,
            gate: 1.0,
        }
    }
}
