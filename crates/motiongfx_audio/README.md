# MotionGfx Audio

Sound events for MotionGfx animations. A note or a sample play is an
ordinary `Animation`, timed by where it sits in `chain`/`all`/`flow`, and
each names its own sound, so one scene can mix synths, SoundFont
instruments and recorded effects.

```rust
use motiongfx::prelude::ms;
use motiongfx_audio::fundsp::prelude32::{adsr_live, sine_hz, square_hz};
use motiongfx_audio::{Instrument, Sample, SoundFont};

let sine = Instrument::synth(|hz| sine_hz(hz) * adsr_live(0.01, 0.1, 0.6, 0.2));
let square = Instrument::synth(|hz| 0.3 * (square_hz(hz) * adsr_live(0.005, 0.05, 0.4, 0.1)));
let piano = SoundFont::load("assets/piano.sf2")?.program(0);
let whoosh = Sample::load("assets/whoosh.mp3")?;

let animation = scene.compile([chain([
    all([swap_bars(a, b), sine.note(key).over(ms(120))]),
    all([highlight(c), square.velocity(0.6).note(key + 7)]),
    all([camera.position().to((0.0, 200.0)), whoosh.gain(0.8).play()]),
    piano.note(60).over(s(1)),
])]);
Window::new("Demo", (1280.0, 720.0)).audio().run(animation)?;
```

## Sounds

- `Instrument::synth(|hz| graph)` (feature `fundsp`, default) builds a
  fundsp graph per note. Its one input is the gate (1 while held, 0 after
  the release), which `adsr_live` reads; it has one or two outputs. The
  crate re-exports `fundsp` so the versions match.
- `SoundFont::load(path)?.program(n)` (feature `rustysynth`) picks preset
  `n` of bank 0 from a `.sf2` file.
- `Sample::load(path)` (feature `symphonia`) decodes mp3, wav, ogg/vorbis
  or flac; `Sample::mono`/`Sample::stereo` wrap generated audio.

Loading is eager and happens on the calling thread; a missing or bad file
is a `LoadError` naming the path. Sounds are cheap to clone and share their
data, so reuse them by passing `&Instrument` into functions that build
animations.

## Notes and plays

- `instrument.note(pitch)` holds the note for 250 ms (`NOTE_LENGTH`), or
  for `.over(d)`. That is its length on the timeline; the release rings on
  afterwards without holding anything up.
- `sample.play()` lasts the sample's duration; `.over(d)` cuts it short
  with a 5 ms fade, or adds silence after it.
- `instrument.velocity(v).note(..)` and `sample.gain(g).play()` set the
  loudness of one event. Both borrow the sound; nothing is copied until
  the event is made. SoundFonts receive the velocity as MIDI velocity.
- `Pitch` is a MIDI note number (60 is middle C, 69 is A4 = 440 Hz), with
  fractions between semitones and `Pitch::hz(..)`. Whole notes convert from
  `u8`, so `note(key + 7)` is a fifth up. SoundFonts round to the nearest
  key.

Events are fire-and-forget. `.interrupt()` cuts property changes, never a
sound, and an event inside `any(..)` plays its full length even when the
group ends sooner.

## Voices

Every event is its own voice, the same for every kind of sound: a fundsp
graph per synth note, a small rustysynth synthesizer per SoundFont note
(so a release can't end another note of the same key), and a separate read
position per sample play (overlapping plays layer). At most 32 voices
sound at once; starting another fades the oldest out over 5 ms. Voices are
summed into stereo and clamped to ±1.

## Playing

`Score::new(&compiled)` collects a compiled scene's sound events.

- `score.render(sample_rate)` renders them offline into interleaved stereo,
  until the scene has ended and every voice has fallen silent. Tests use
  this; no audio device is needed.
- `Player::new(score, sample_rate)` returns a `Player` for the host thread
  and a `Mixer` for any audio callback. Each frame the host calls
  `player.advance(from, to)` while time moves, and `player.stop_all()` on a
  seek, pause or change of direction. Backwards (`to` before `from`), the
  score plays mirrored: each event starts as the playhead reaches its end,
  so notes come in reverse order, each still sounding forwards. Events start at their
  exact sample offset, a constant 30 ms behind the clock. Loop wraps
  continue seamlessly when reported as `advance(previous, end)` then
  `advance(loop_start, now)`.
- `AudioOutput::open(score)` (feature `cpal`) does the same through the
  default output device.

The mixer never allocates, frees, locks or blocks: voices are built on the
host thread, sent over `rtrb` queues and sent back to be freed there.

Visual sampling is unaffected: sound is a side channel driven by the play
clock, and scrubbing never plays sound. Instruments are closures and are
not serializable. See [the design note](../../docs/audio.md) for the
reasoning and limitations.
