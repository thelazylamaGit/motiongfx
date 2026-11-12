# Vello / Winit examples

`moving_square`, `insertion_sort`, `commit_graph`, `camera_zoom`, `sine_unit_circle`
and `bezier_explainer` / `radius_area` use the plain native
authoring API.
The optional `animate!` macro remains available, but these examples do not
use it. The other comparison scenes retain the low-level MotionGfx API.
All share the same play/pause and
scrubbing controls from `vello_winit_motiongfx`:

| Example | Source | What it exercises |
| --- | --- | --- |
| `insertion_sort` | Canvas Commons sorting | Procedural creation, nested position targets, moving text labels, discrete pass captions |
| `insertion_sort_low_level` | Previous annotated version | Same sorting choreography using raw MotionGfx and Vello calls |
| `commit_graph` | Canvas Commons commit graph | A reusable circle/ring component, path reveal, staggered back easing, ordinary ring scale/opacity tracks |
| `moving_square` | Manim `MovingAround` | Typed property handles, sequential translation, fill interpolation, relative scale, and rotation |
| `camera_zoom` | Manim-style `Zoom` | Parallel camera pan/zoom, a hold, and returning to the original view |
| `sine_unit_circle` | Manim `SineCurveUnitCircle` | Native typed bindings, dependent line endpoints, and cached path reveal |
| `bezier_explainer` | Quadratic Bézier lesson | Typed signals/computed values, source-aware object creation, exact quadratic geometry, and ping-pong playback |
| `radius_area` | Motion Canvas radius/area signals | One animated radius drives circle size, a dashed double-arrow line, label positions and computed area text |

```sh
cargo run -p vello_winit_example --example commit_graph
cargo run -p vello_winit_example --example moving_square
cargo run -p vello_winit_example --example camera_zoom
cargo run -p vello_winit_example --example sine_unit_circle
cargo run -p vello_winit_example --example bezier_explainer
cargo run -p vello_winit_example --example radius_area
```

The radius/area example mirrors the supplied signal relationships: radius
starts at 3, grows to 4 over 2s, returns to 3 over 2s, then holds for 1s.
It uses cubic in/out easing, matching Motion Canvas's signal tween default
([implementation](https://github.com/motion-canvas/motion-canvas/blob/main/packages/core/src/signals/SignalContext.ts)).
The displayed area is `PI * r * r`, in mathematical units; only geometry uses
the 100px/unit scale. Source y-down label positions are converted to y-up,
with a 20px top inset replacing the source's text padding. The radius line
uses reusable native dash/arrow/endpoint-trim support, with its tip 8px inside
the circle. Text uses the existing cached 5x7 lettering, not a font renderer:
font weight 700 and the source font metrics are not reproduced.
`scene.set_background(...)` selects a light clear color without a background
rectangle, making the supplied red/dark annotation colors easy to inspect.
No explicit bindings or update callbacks are needed in the example.

The commit graph preserves the source's blue/mauve fallbacks and timing
(3.88s total). `Path` creates the trunk and fork explicitly, with their visible
end initially zero. A local `Commit` constructor creates a filled `Circle`
and a stroked child ring, anchored with `relative_to(dot)`. `sequence.stagger`
animates only the dots' scales; the rings inherit the appearance transform.
The pulse uses ordinary ring scale (1 to 2.4) and opacity (1 to 0) tracks in
parallel, equivalent to the source's linear pulse expressions. No pulse-specific
library binding or general signal evaluator is introduced. Coordinates are
converted from Canvas Commons' y-down to this API's y-up convention.

The square uses one-second actions with Manim's `smooth`
easing (4s total); changing its fill leaves its blue stroke unchanged.
The authoring crate supplies those duration/easing defaults, and the shared
palette supplies `BLUE`/`ORANGE`; the example only declares overrides.
The sine scene uses the same 0.25 turns/second for 8.5s, then holds for the
source's final default one-second wait. It precomputes a `Path` and animates
its `end`, rather than accumulating segments in a dt updater. `scene.bind`
calculates the dot position from the sampled reveal. `Line` endpoints follow
the dot and a tuple of reveal/y inputs, using native Rust functions. Bindings
are ordered/validated during construction and skip calculations for unchanged
inputs. The same relationships reconstruct when seeking backwards. There are
no manual registry, world, sampler, geometry cache, or window implementations
in this example. `try_animate` reports binding errors before opening the window.
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

Read `examples/bezier_explainer.rs` in order: reusable grid construction,
signals/computed geometry, object creation, then the timeline. It uses
standalone `signal(...)` for guide opacity, probe position and bend. `.map(...)`
and `computed!` describe shared calculations without borrowing the scene. Computed
positions feed circles, connecting lines and the exact quadratic directly in
their constructors; no `scene.update` or explicit `scene.bind` calls are used.
The reusable grid helper creates styled lines whose `reveal` properties are
animated together in the timeline without procedural point-buffer updates.

The lesson keeps its original 14.4-second timing and automatic reverse playback,
using `Window::playback_mode(PlaybackMode::PingPong)`. An exact quadratic replaces
the old 180-segment curve; changing bend updates its control point, while holds
reuse geometry. Labels are anchored `Text` objects. Chapter changes are discrete
timeline events, so changing durations does not require fixing time thresholds.
The readout is computed from `t` and `scene.time()`; the shared controls display
pause/reverse status. The scene footer gives input instructions.

Start experimenting with the control points, relative bend offsets, or action
durations. Coordinates now use the shared authoring convention: centered, with
positive y upward. Signal/builder type safety, exact curve math, cache reuse and
backward seeking are covered by reusable backend tests:

```sh
cargo test -p vello_motiongfx
```

To include the shared clock/control tests and all demos:

```sh
cargo test -p vello_winit_example --all-targets
```

The existing, more elaborate Lissajous table is also available:

```sh
cargo run -p vello_winit_example --example lissajous
```
