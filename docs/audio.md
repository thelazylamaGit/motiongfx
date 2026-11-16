# Audio: sound events in animations

Status (2026-11-17): **implemented** (core cues, authoring events,
`motiongfx_audio`, the host's `Window::audio()`, the `sorting` example).
Tested headlessly; not yet listened to through a device. API details are
in the [crate README](../crates/motiongfx_audio/README.md); this note
records the design and why.

## Goal

Sounds are part of the animation, not a host setting. A sound event is an
ordinary `Animation`, timed by where it sits in `chain`/`all`/`flow`, and it
names its own sound. One scene can play a sine note here, a square note
there, a SoundFont piano elsewhere, and an mp3 effect at a chosen moment.

```rust
let sine = Instrument::synth(|hz| sine_hz(hz) * adsr_live(0.01, 0.1, 0.6, 0.2));
let square = Instrument::synth(|hz| square_hz(hz) * adsr_live(0.005, 0.05, 0.4, 0.1));
let piano = SoundFont::load("assets/piano.sf2")?.program(0);
let whoosh = Sample::load("assets/whoosh.mp3")?;

scene.compile([chain([
    all([swap_bars(a, b), sine.note(key).over(ms(120))]),
    all([highlight(c), square.velocity(0.6).note(key + 7)]),
    all([camera.position().to((0.0, 200.0)), whoosh.play()]),
    piano.note(60).over(s(1)),
])])
```

## Layers

| Layer | Adds | Knows about audio? |
| --- | --- | --- |
| `motiongfx` (core) | **cues**: numbered points in a `TrackFragment`, shifted by the combinators like clips. They replaced core's named markers. | no |
| `motiongfx_authoring` | `event(payload, length)`; markers and events lower to cues; `compiled.events::<E>()` | no (any `Send + Sync + 'static` payload) |
| `motiongfx_audio` | `Pitch`, `Instrument`, `SoundFont`, `Sample`, `Score`, voices, mixing, offline render, `Player`/`Mixer`, cpal `AudioOutput` | yes |
| `vello_winit_motiongfx` | `Window::audio()` (feature `audio`): opens the device and drives it from the clock | only through `motiongfx_audio` |

### Core: cues instead of markers

Authoring can't work out absolute times itself: lowering produces relative
fragments that core's `chain`/`all`/`any`/`flow`/`delay` shift later, and
recomputing those offsets would duplicate the combinator rules. Markers
were in core for that reason. Rather than add a second parallel mechanism,
core's named markers became nameless cues that both markers and events use:

```rust
pub struct CueId(pub u32);              // chosen by the caller
TrackFragment::cue(id) / add_cue(id, time) / cues()
Track::cues() -> &[(CueId, Duration)]   // sorted by time, stable
```

Core lost `MarkerName`, the name lookup and its strings; nothing outside
authoring used them. A compiled track still reaches its last cue. Reusing
markers with generated names was rejected: a string per event, and
internal names in `compiled.markers()`.

### Authoring: one cue table for markers and events

- `marker(name)` keeps its own leaf for its rules (unique names, zero
  length, no slot in `flow`) and lowers to a cue. `MarkerName` moved here.
  The public marker API is unchanged.
- `event(payload, length)` is a node `Event(Arc<dyn Any + Send + Sync>,
  Duration)` (within the 48-byte `Animation` size check). It lowers to a
  cue followed by `length` of silent time. Each use gets its own cue, so a
  cloned or reused event fires once per place.
- `CompileContext` keeps one table from cue ID to marker name or event.
  After `track.compile()`, `try_compile` splits `track.cues()` into sorted
  markers and sorted events stored in `CompiledScene`.
  `compiled.events::<E>()` filters by `downcast_ref` and yields
  `Event { start, length, payload: &E }`.

**Events take their length on the timeline**, like every other animation:
`chain([note, note])` is a melody, and `all([move, note])` lasts as long as
the longer. To keep a note from lengthening a step, put it in `any` or
`flow` (the sorting sweep does).

**Timing modifiers:** `.over(d)` sets an event's length (without it, the
length given to `event`, which the creating crate chooses: 250 ms for a
note, the duration for a sample, not the scene's movement default).
`.ease`/`.spring` panic at the caller's line. `.interrupt()` leaves events
alone: interruption cuts property clips, and events have no property, so
**sound events are fire-and-forget**. Inside `any(..)` an event plays its
whole length even when the group ends sooner.

## `motiongfx_audio`

Depends on `motiongfx_authoring` only, so `note`/`play` return
`Animation<M>` for any model.

### API decisions

- **Length is `.over(d)`; loudness sits on a borrowed builder.** `.note()`
  must return a plain `Animation` to share an array with other changes,
  and another crate can't add methods to `Animation` (an extension trait
  would put `.velocity()` on every animation, compile `bar.fill_to(..)
  .velocity(..)`, and need an import). So it's
  `sine.velocity(0.6).note(key)` and `whoosh.gain(0.5).play()`, where
  `velocity`/`gain` return `AtVelocity`/`AtGain`: a reference plus a
  number. Nothing is copied until the event is made, which copies the
  sound's `Arc` once, as every event must. Each event can have its own
  loudness; changing loudness during one playback (automation) isn't
  supported.
- **`SoundFont::load(path)?.program(n)`**: one file, many programs, and it
  mirrors `Sample::load`. A missing program panics at the caller's line.
- **`Pitch` is a fractional MIDI number** with `Pitch::hz` and `From<u8>`.
  Synths get Hz; SoundFonts round to a key.
- **`synth` takes a closure** building one fundsp graph per note (not
  serializable; serialization is deferred). Input 0 is the gate; one or two
  outputs. `synth` probes the closure at 440 Hz and panics at the caller's
  line on other shapes. fundsp is re-exported because its types are in the
  signature; rustysynth/symphonia types stay private.
- **`Sample::mono`/`stereo`** wrap generated audio. Tests use them, and so
  can procedural sounds; `load` needs the `symphonia` feature.
- `Instrument` exists only with `fundsp` or `rustysynth`, and `LoadError`
  only with `rustysynth` or `symphonia`, so no feature set has an
  unconstructible type.
- **Loading is eager**, on the calling thread; samples are stored as
  interleaved stereo `f32` at their own rate.

### Playback

`Score::new(&compiled)` collects the private payload type's events.
`score.render(rate)` renders offline. `Player::new(score, rate)` returns the
host-thread `Player` (`advance(from, to)`, `stop_all()`) and the
audio-thread `Mixer` (`render(&mut [f32])`), usable from any callback.
`AudioOutput` (feature `cpal`) wraps both around the default device. The
cpal glue lives here rather than in the window host so other hosts can use
it.

**Real-time rules.** The mixer never allocates, frees, locks or blocks.
Voices are built on the host thread in `advance`. They move *by value*
through preallocated `rtrb` queues into fixed-capacity vectors, and back to
the host thread to be dropped there. Boxing them would add an allocation
per note and gain nothing. Full queues skip an event instead of blocking.

**Timing.** The mixer publishes its rendered frame count through an atomic.
The first `advance` after `stop_all` anchors at "rendered + 30 ms". Every
event starts at `anchor + (play time since the anchor)`, computed in exact
integer frames, so spacing is sample-accurate whatever the frame rate.
Successive `advance` calls continue from the anchor even when `from` jumps,
which keeps loop wraps seamless. Sound runs a constant 30 ms (plus device
buffering) behind the picture. Device and host clock drift is not
corrected.

**Voices**, one per event:
- synth: a fundsp graph, gate 1 then 0 at note-off. fundsp's `adsr_live`
  starts its attack only when the gate *rises*, so each graph is ticked once
  with the gate low when it's built (on the host thread; that frame isn't
  heard);
- SoundFont: one small rustysynth `Synthesizer` per note (no reverb or
  chorus, polyphony 8 for layered zones). A release can't end another note
  of the same key; that's the MIDI note-off-by-key quirk, avoided without
  channel juggling;
- sample: a read position into the shared frames, linearly resampled;
  overlapping plays layer.

Released synth/SoundFont voices end after 50 ms below −80 dB, or 10 s at
most; samples end with their data, or fade over 5 ms at `start + length`.
A synth graph with no gate input fades at note-off. Above 32 voices the
oldest fades out over 5 ms (a hard cut clicks). The mix is clamped to ±1.

**Seek, pause, reverse.** `stop_all` fades every voice over 5 ms and drops
waiting ones. Resuming starts only events after the new time. Notes already
under way aren't restarted. Visual sampling never depends on sound.

### Dependencies

| Feature | Crate | Why |
| --- | --- | --- |
| `fundsp` (default) | fundsp 0.23 (Jan 2026, MIT/Apache) | Composable graphs, pure Rust, `AudioUnit` per voice; no assets. Built without its `files`/`fft` features. |
| `rustysynth` | rustysynth 1.3.8 (Oct 2026, MIT) | Pure-Rust SF2 synth with a small API. oxisynth is LGPL-2.1 and less active. |
| `symphonia` | symphonia 0.6.1 (Aug 2026, MPL-2.0) + `mp3` | The standard pure-Rust decoder. MPL is file-level copyleft; using it unmodified doesn't affect this crate's licence. |
| `cpal` | cpal 0.18.2 (Aug 2026, Apache-2.0) | The standard output crate, with device and format negotiation and errors. tinyaudio hides both. f32 and i16 devices are supported. |
| always | rtrb 0.4.1 (Oct 2026, MIT/Apache) | Wait-free SPSC queues. |

## Host

`Window::new(..).audio().run(animation)`. `run` builds a `Score`. No sound
events means no device. If the device fails to open, the preview runs
silently and prints why: a preview without sound beats none, and `run`'s
error type belongs to the event loop. The app keeps a private
`AudioSync`; `tick_playback` passes it the clock state just before and just
after each advance. A pure function turns that into player calls:

- moving forwards: `advance(before, after)`; after a pause or on the first
  tick, `stop_all` first so the player re-anchors;
- a loop wrap: `advance(before, end)` then `advance(loop_start, after)`;
- ping-pong turning: advance to the end, then stop; nothing plays backwards;
- not moving: `stop_all` if anything changed since the last tick (pause,
  seek, focus loss). Reaching the end of a single play changes nothing, so
  its last notes ring out.

## Sorting example

`build_animation(&Instrument)`: `main` picks a SoundFont from the first
argument, otherwise a quiet sine. Each cursor step is
`all([fade chain, instrument.note(bar_note(value)).over(NOTE_TIME)])
.delay(clock).interrupt()`. Pitch follows height on a three-octave
pentatonic scale from C3, and notes last 80 ms. The final sweep puts the
note inside the existing `any` so it doesn't slow the sweep.

Offline render of the whole scene (release build, 48 kHz): 1447 notes over
34.7 s, peak 0.525, RMS 0.126, nothing clipped. It rendered in 85 ms (about
400× real time).

## Tests

- Core: cues move with their fragments; compiled tracks sort and reach
  them.
- Authoring: event start times in chains, `all`, `flow`, `.delay`, `.over`,
  `any`; reused and interrupting events each fire; payload filtering and
  markers alongside; `.ease` on an event panics.
- Audio (no device):
  - exact-frame starts;
  - layering plays;
  - a same-pitch overlap where the first release leaves the second intact;
  - stealing the oldest beyond 32 voices;
  - `stop_all` fading, and a resume starting only later events;
  - loop-wrap spacing;
  - render length and silence after tails;
  - velocity scaling;
  - misuse panics;
  - a WAV written by the test, decoded and resampled;
  - a missing file's error;
  - a minimal SoundFont generated by the test (one looped sine), playing
    and keeping a same-key overlap intact.
- Host: the clock-to-player rules above.
- Example: `builds_and_samples_headlessly`.

## Not included

Pitch bend or glides, per-note pan, volume automation within a sound,
effects buses, streaming long files, audio export alongside video, clock
drift correction, a limiter, and serializable instruments.
