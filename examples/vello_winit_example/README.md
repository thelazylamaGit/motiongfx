# Vello / Winit examples

`moving_square` and `insertion_sort` use the plain native authoring API.
The optional `animate!` macro remains available, but these examples do not
use it. The other comparison scenes retain the low-level MotionGfx API.
All share the same play/pause and
scrubbing controls from `vello_winit_motiongfx`:

| Example | Source | What it exercises |
| --- | --- | --- |
| `insertion_sort` | Canvas Commons sorting | Procedural creation, nested position targets, moving text labels, discrete pass captions |
| `insertion_sort_low_level` | Previous annotated version | Same sorting choreography using raw MotionGfx and Vello calls |
| `commit_graph` | Canvas Commons commit graph | A reusable `Commit` component, path tracing, staggered back easing, derived ring opacity/scale |
| `moving_square` | Manim `MovingAround` | Typed property handles, sequential translation, fill interpolation, relative scale, and rotation |
| `sine_unit_circle` | Manim `SineCurveUnitCircle` | Orbiting point, dependent lines, and sine-path tracing |

```sh
cargo run -p vello_winit_example --example commit_graph
cargo run -p vello_winit_example --example moving_square
cargo run -p vello_winit_example --example sine_unit_circle
```

The commit graph preserves the source's blue/mauve fallbacks and timing
(3.88s total). The square uses one-second actions with Manim's `smooth`
easing (4s total); changing its fill leaves its blue stroke unchanged.
The authoring crate supplies those duration/easing defaults, and the shared
palette supplies `BLUE`/`ORANGE`; the example only declares overrides.
The sine scene uses the same 0.25 turns/second for 8.5s, then holds for the
source's final default one-second wait. It precomputes the sine path and
animates `PathTracer.t_end`, rather than accumulating segments in a dt
updater. The sampled trace progress also positions the dot and connecting
lines. This makes scrubbing reversible with the existing tracing support.
Its four `MathTex` labels are represented by ASCII `PI` labels; no new
typesetting dependency is introduced for these baseline comparisons.

The square creates a `Rectangle` once and schedules `position.offset`,
`fill.to`, `scale.multiply`, and `rotation.offset` on its returned handle.
`Scene::animate` executes once to build and bake an ordinary MotionGfx
timeline. The host samples it and renders the stored rectangle each frame.
Coordinates are centered logical canvas units with positive y upward;
rotation is counterclockwise radians. There is no scripting or expression
graph evaluator. The low-level-equivalence and seek tests now live in
`crates/vello_motiongfx/src/lib.rs`.

The **insertion sort** example recreates the supplied Canvas Commons scene
using typed rectangle/text handles and `Sequence::play_all`:

```sh
cargo run -p vello_winit_example --example insertion_sort
cargo run -p vello_winit_example --example insertion_sort_low_level
```

It uses `[5, 2, 8, 1, 6, 3]`, 64px-wide rounded bars, height `value * 36`,
a shared bottom baseline, 90px slot spacing, and the `#89b4fa` fallback
color. Each of the five insertion passes animates every bar in parallel
for 400ms with cubic ease-in/out, followed by a 600ms final hold (2.6s total).
The source's theme-variable lookup is represented by that fixed fallback.
The algorithm runs during construction; scrubbing only samples the compiled
timeline. Edit the `VALUES` slice to change the number or values of the bars.
Only the handle ordering changes during construction; the stored objects
keep their identities and initial geometry. This scene explicitly overrides
the library's duration/easing defaults with 400ms and cubic ease-in-out.

The title, dynamic input caption, per-bar value labels, and pass caption are
`Text` scene objects. Each value label is anchored to its rectangle with
`relative_to(bar)` and a local position. Only the rectangle's `position.x`
is animated; labels inherit the movement automatically. The sorting order
stores `(value, rectangle_handle)` pairs, without a separate component wrapper
or label animation tracks. The pass caption uses
`sequence.play(status.content.set(...))`, so seeking also restores its text.
The ordinary Rust sorting loop schedules all bar moves together with `play_all`.
The example contains only scene creation and choreography; reusable-crate tests
cover nested properties, anchoring, text changes, rendering caches and seeking.
The initial text renderer is still cached ASCII vector lettering, not a real
font/shaping engine. The original annotated low-level version remains as
`insertion_sort_low_level` for comparisons.

Run from the workspace root:

```sh
cargo run -p vello_winit_example --example timeline_wave
```

The **timeline wave** is the small MotionGfx authoring reference. A single
timeline grows the wave, animates phase/frequency/marker radius together,
holds, reshapes it, moves it backward, and flattens it. Holds actually stop
the wave because geometry reads sampled properties, not wall-clock time.
Its path buffer and styles are created once, and geometry is only updated
when the sampled time or viewport changes. It uses the existing MotionGfx
API with no speculative wrappers or macros. The preview duration comes
from the compiled timeline.

For the larger mathematical explainer:

```sh
cargo run -p vello_winit_example --example bezier_explainer
```

The **Bezier explainer** draws a grid and three control points, then
demonstrates de Casteljau's construction. Gold points A and B slide along
the control polygon. Pink point Q slides between A and B, tracing the
teal quadratic curve. Moving P1 bends the entire construction. Playback
automatically reverses so you can see that the same timeline works in
both directions. Close the window to exit.

The shared Winit host provides these controls for **all** examples:

- Click or drag the bottom timeline bar to scrub. Scrubbing pauses playback.
- Press **Space** to pause or resume.
- Press **Left/Right** to seek backward/forward by 0.25 seconds and pause.
- Press **Home/End** to jump to the beginning/end and pause.
- Click the play/pause icon at the left of the control strip.

After a manual seek, playback resumes forward. Losing window focus pauses
the animation. The Bezier demo automatically reverses at the end;
Lissajous plays its introduction once and then loops its curve chapter.

The controls are independent of `motiongfx::Timeline`. Each demo receives
a `PlaybackState` in `rebuild_scene` and uses its absolute `time`. A
timeline demo calls `set_target_time(playback.time)`; a procedural demo
can simply evaluate its own math at `playback.time.as_secs_f64()`.

Override `VelloDemo::playback_config` to choose the preview duration and
mode (`Once`, `Loop`, `PingPong`, or `LoopFrom`). The default is a 30-second
preview that stops at the end. The range can be any useful test interval;
it doesn't have to be a timeline's duration. The host reserves a separate
control strip, so each demo's `size_changed` still describes its drawable
area and doesn't need to know how the controls are laid out.

For a concrete example using **no MotionGfx timeline**, run:

```sh
cargo run -p vello_winit_example --example procedural_wave
```

Its wave is a pure function of time, so scrubbing reconstructs the picture
directly. Stateful simulations need their own reset/replay or checkpoints
to reconstruct earlier states; the clock cannot make them seekable alone.

Read `examples/bezier_explainer.rs` in its numbered sections:

1. **Math:** three linear interpolations generate a quadratic curve.
2. **World:** a plain struct and `SubjectSource` connect data to MotionGfx.
3. **Timeline:** typed paths, target functions, easing, parallel/serial
   composition, relative actions, silent pauses, compilation, and baking.
4. **Drawing:** sampled scalar parameters produce all the geometry.
5. **App:** sample an absolute time, fit the canvas, and render with the
   existing window/GPU harness.

Start experimenting with the three positions in `Diagram::points`, the
relative offsets in `build_timeline`, or the action durations. The gold
line and pink marker are computed from the current control points every
frame, so they remain consistent as you change the shape. Coordinates
are screen coordinates: positive y points down.

`src/labels.rs` is only a small font-free label renderer;
you can skip it while learning MotionGfx. The numerical construction and
backward seeking can be checked without opening a window:

```sh
cargo test -p vello_winit_example --example bezier_explainer
```

To include the shared clock/control tests and all demos:

```sh
cargo test -p vello_winit_example --all-targets
```

The existing, more elaborate Lissajous table is also available:

```sh
cargo run -p vello_winit_example --example lissajous
```
