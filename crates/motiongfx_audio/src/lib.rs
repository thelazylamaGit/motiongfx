//! Sound for MotionGfx animations.
//!
//! Sounds are part of the animation: a note or a sample play is an
//! ordinary [`Animation`](motiongfx_authoring::Animation), timed by where
//! it sits in `chain`/`all`/`flow`, and each names its own sound. Create
//! sounds in scene code like signals:
//!
//! - [`Instrument::synth`] builds a fundsp graph per note (feature
//!   `fundsp`, on by default);
//! - [`SoundFont::load`] reads a `.sf2` file, and
//!   [`program`](SoundFont::program) picks an instrument from it (feature
//!   `rustysynth`);
//! - [`Sample::load`] decodes an mp3/wav/ogg/flac file (feature
//!   `symphonia`), and [`Sample::mono`]/[`Sample::stereo`] wrap generated
//!   audio.
//!
//! `instrument.note(pitch)` and `sample.play()` return animations;
//! `.over(..)` sets how long they last, and `instrument.velocity(..)` and
//! `sample.gain(..)` how loud.
//!
//! After `scene.compile(..)`, [`Score::new`] collects the scene's sound
//! events. [`Score::render`] renders them offline; [`Player`] and
//! [`Mixer`] play them live from any audio callback, and [`AudioOutput`]
//! (feature `cpal`) plays them through the default device. Sound plays only
//! during forward playback: hosts call [`Player::stop_all`] on a seek or
//! pause. Sampling the scene's pictures never depends on sound.
//!
//! # Examples
//!
//! ```
//! use motiongfx::prelude::ms;
//! use motiongfx_audio::fundsp::prelude32::{adsr_live, sine_hz, square_hz};
//! use motiongfx_audio::{Instrument, Score};
//! use motiongfx_authoring::{Scene, all, chain, signal};
//!
//! let sine = Instrument::synth(|hz| sine_hz(hz) * adsr_live(0.01, 0.1, 0.6, 0.2));
//! let square = Instrument::synth(|hz| 0.3 * (square_hz(hz) * adsr_live(0.005, 0.05, 0.4, 0.1)));
//! let x = signal(0.0);
//!
//! let scene = Scene::<()>::new().compile([chain([
//!     all([x.to(100.0), sine.note(60).over(ms(120))]),
//!     all([x.to(0.0), square.velocity(0.6).note(67)]),
//! ])]);
//! let audio = Score::new(&scene).render(48_000);
//! assert!(audio.iter().any(|&sample| sample != 0.0));
//! ```
#![warn(missing_docs)]

#[cfg(feature = "symphonia")]
mod decode;
mod engine;
#[cfg(any(feature = "fundsp", feature = "rustysynth"))]
mod instrument;
#[cfg(any(feature = "rustysynth", feature = "symphonia"))]
mod load_error;
#[cfg(feature = "cpal")]
mod output;
mod pitch;
mod player;
mod sample;
mod score;
mod sound;
#[cfg(feature = "rustysynth")]
mod soundfont;

/// The fundsp version [`Instrument::synth`] takes graphs from.
#[cfg(feature = "fundsp")]
pub use fundsp;
#[cfg(any(feature = "fundsp", feature = "rustysynth"))]
pub use instrument::{AtVelocity, Instrument, NOTE_LENGTH};
#[cfg(any(feature = "rustysynth", feature = "symphonia"))]
pub use load_error::LoadError;
#[cfg(feature = "cpal")]
pub use output::{AudioOutput, OutputError};
pub use pitch::Pitch;
pub use player::{Mixer, Player};
pub use sample::{AtGain, Sample};
pub use score::Score;
#[cfg(feature = "rustysynth")]
pub use soundfont::SoundFont;

// The mixer moves to the audio thread, and errors cross threads.
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send::<Mixer>();
    assert_send::<Player>();
    assert_send_sync::<Sample>();
    #[cfg(any(feature = "rustysynth", feature = "symphonia"))]
    assert_send_sync::<LoadError>();
    #[cfg(any(feature = "fundsp", feature = "rustysynth"))]
    assert_send_sync::<Instrument>();
};
