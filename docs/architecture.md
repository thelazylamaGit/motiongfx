# Native authoring architecture and direction

This note records the current native authoring work as of 2026-11-14. It is a
handoff for contributors and coding agents, not a commitment to implement every
future feature below. Update it alongside architectural changes.

## Goal and scope

The immediate goal is readable Rust animation code: explicitly create objects,
compose reusable helpers and procedural math, describe changes, then preview
with Vello/Winit. Motion Canvas and Manim are API references, not requirements
to copy JavaScript/Python semantics into Rust.

MotionGfx remains the timeline foundation because it already provides relative
actions, sequencing/parallel/stagger ordering, interpolation, and arbitrary-time
sampling. Cleaner authoring should remove repeated plumbing while retaining
those capabilities. An example reduced to `new_explainer().progress.to(1)` does
not demonstrate a general authoring API.

Longer-term interests include richer text/code animation, interactive effects,
an editor, and potentially scripting with hot reload. The present focus is the
native Rust authoring API. There is no decision to replace it with a scripting
engine, build a complete editor, or introduce ECS.

## Current crate responsibilities

| Crate | Responsibility and boundary |
| --- | --- |
| [`motiongfx`](../crates/motiongfx/README.md) | Typed actions, timelines, ordering, baking and sampling; independent of graphics/windowing. |
| [`motiongfx_interp`](../crates/motiongfx_interp/README.md) | Interpolation support. |
| [`motiongfx_authoring`](../crates/motiongfx_authoring/README.md) | Shared `Scene<M>`/`CompiledScene<M>` lifecycle, registration/property preparation, sequences, signals/computations and native binding/update execution. No graphics/window dependency. |
| [`motiongfx_2d`](../crates/motiongfx_2d/README.md) | Portable 2D objects/storage, geometry, styles, typed handles, attachments and camera semantics. Uses Kurbo/Peniko directly, not Vello/Winit. |
| [`peniko_motiongfx`](../crates/peniko_motiongfx/README.md) | Geometry/color interpolation and path-related integration; not a window host or complete scene renderer. |
| [`motiongfx_text`](../crates/motiongfx_text/README.md) | Shared Parley font/layout contexts, embedded/registered fonts, positioned glyphs, UTF-8 cluster ranges and layout bounds; no scene/window/GPU. |
| [`motiongfx_audio`](../crates/motiongfx_audio/README.md) | Sound events (notes on synth/SoundFont instruments, sample plays) as `Animation`s, voices, mixing, offline rendering and live playback (`Player`/`Mixer`; cpal output behind a feature). Depends on authoring only; no window or graphics. |
| [`vello_motiongfx`](../crates/vello_motiongfx/README.md) | Vello drawing and renderer-owned caches. Consumes portable sampled models; exports only renderer types (objects come from `motiongfx_2d`). |
| [`vello_winit_motiongfx`](../crates/vello_winit_motiongfx/README.md) | Native window/GPU setup, resize/DPI handling, playback clock, play/pause and scrubbing controls. These are shared host facilities, not required scene objects. With feature `audio`, `Window::audio()` drives `motiongfx_audio` from the clock. |
| [`zgui_editor`](../crates/zgui_editor/README.md) | A zgui editor window with the same `Window` builder: Vello renders the sampled scene into a zgui `SurfaceRenderer` on zgui's own wgpu device, below which zgui widgets give the timeline. Reuses the playback clock and audio sync from `vello_winit_motiongfx`. Depends on a local zgui fork checkout (`../../zgui`). The examples use it with their `zgui` feature. |

The two hosts have different jobs. `vello_winit_motiongfx` is the
lightweight player: it is what a distributed single animation and the
everyday example runs use, so it stays small in binary size and compile
time, with its controls drawn by Vello and no UI framework. `zgui_editor` is
the start of a larger editor and may grow its own playback model; it depends
on the player crate, never the other way round, so editor work costs the
player nothing.
| [`motiongfx_scene`](../crates/motiongfx_scene/README.md) | Serializable scene/timeline documents, named registries and compilation into core actions. Distinct from live `motiongfx_authoring::Scene<M>`; not currently a dependency of native authoring. |
| [`bevy_motiongfx`](../crates/bevy_motiongfx/README.md) | Separate Bevy integration. The Vello/Winit authoring path does not require Bevy. |
| [`velyst_motiongfx`](../crates/velyst_motiongfx/README.md) | Existing Bevy/Velyst/Typst integration, currently excluded from the workspace pending dependency compatibility. Not yet the text implementation for this Vello host. |

The shared-scene/model/renderer separation is implemented. See
[backend-separation.md](backend-separation.md) for its contracts, validation and
remaining integration limits. There is no separate signals or language-binding
crate; further splits should have concrete responsibilities.

Implementation entry points:

- [Standalone source descriptions](../crates/motiongfx_authoring/src/source.rs),
  [signals and storage](../crates/motiongfx_authoring/src/signals.rs), and
  [prepared dependency evaluation](../crates/motiongfx_authoring/src/bindings.rs).
- [Generic descriptors](../crates/motiongfx_authoring/src/object.rs),
  [portable builders](../crates/motiongfx_2d/src/object.rs),
  [reactive input registration](../crates/motiongfx_authoring/src/input.rs),
  [shared finalisation/sampling](../crates/motiongfx_authoring/src/scene.rs), and
  [Vello rendering](../crates/vello_motiongfx/src/shape_renderer.rs).
- [Direct update target borrowing](../crates/motiongfx_2d/src/updates.rs) and
  [native host](../crates/vello_winit_motiongfx/src/lib.rs).

## Shared model boundary

Authoring owns `Scene<M>`, `CompiledScene<M>`, and `World<M>`. World wraps an
application-selected model with signal storage and sampled time. Model crates
implement `WorldObject<ModelId, M>` for each object type, using their own IDs;
one blanket impl turns that into core's `SubjectSource` for `World<M>`.
2D handles are one generic type, `Handle<T>` (typed by the object, not a
type-state). Capability traits (`Transformable`, `Fillable`, `Strokable`,
`Fadable`) are implemented by object types, and bounded inherent impls such
as `impl<T: Transformable> Handle<T>` give every matching handle its methods,
so concrete code needs no trait imports and no getter exists twice. The
traits require `WorldObject<T::Id, Canvas>`, so a function over
`Handle<T>` with `T: Transformable` can call `.to(..)` on its fields without
its own `where World<M>: SubjectSource<..>` clause; generic code for a
composite model `M` adds `T: WorldObject<T::Id, M>`.
Shared code never enumerates graphics/3D types or invokes a renderer.

The 2D convenience aliases specialise that lifecycle to `Canvas`. Local
`Object<T, M>` and `Input<T, M>` wrappers add inherent builders and geometry
conversions over generic `ObjectDefinition` / `Input` preparation in authoring.
They satisfy Rust's inherent-implementation/orphan rules while keeping one
preparation implementation. Detached builders may need an explicit
`Object<Circle>` type when no scene provides inference.

`Model2d` exposes a contained Canvas store in composite models. Ordinary
2D objects and another model can share signals, computed relationships and one
core schedule. This is tested with actual 2D objects plus a small second model.
There is no 3D renderer or automatic Bevy adapter. Custom mixed callback targets
still require a safe model adapter; provided 2D array/tuple borrowing targets
Canvas. No ECS or general dynamic world registry is introduced.

There are no 2D extension traits on `Scene`/`CompiledScene`. Sampled state
is read with the shared `compiled.get(handle)`, typed by the authoring
`ObjectHandle` trait that 2D handles implement. The camera is an ordinary
object (`scene.add(Camera::new())`) created with `ObjectDefinition::unique`, so
adding another replaces the earlier one, including its signal bindings.

The background is a model setting, not an object: the colour a renderer clears
with. Authoring's `Background` trait lets a model name its colour type (and
how to blend it); `Canvas` uses Peniko's. Scenes whose model implements it get
`scene.set_background(..)` (fixed, signal or computed; the last call wins; the
result animates with `.to(..)`) and `compiled.background()` as ordinary
methods, so authoring stays free of graphics types.

## Construction, sampling, rendering

1. Constructors such as `Rectangle::new` return `Object<T>` descriptions.
   Fixed styles/defaults are initial state. Reactive setters retain typed inputs.
   Adding endpoint shapes produces a typed `LineWithTips` construction wrapper.
   Its handle exposes ordinary line properties and the concrete shape handles.
2. Built-in field targets use `Target<I,S,T,F>` and zero-sized field markers.
   A target stores its object ID; field accessors/interpolation are supplied by
   the marker type when expanded into the general `Property` representation.
   Object handles themselves now store just one ID; getter methods construct
   individual targets on demand. Rectangle/Circle/Text/Line/Path/Triangle/Chevron
   handles measure 8 bytes on 64-bit targets, replacing the prior property bundles
   (104/88/136 bytes for Rectangle/Circle/Text). The single camera's handle is
   zero-sized. No extra target fields or pointer allocations are retained in
   handles. Actions, bindings and springs use existing preparation/playback;
   generic runtime paths keep Property. This is a storage reduction, not a
   measured frame-time/build-time improvement. Index/scene safety rules remain.
   Field declarations live beside their owning objects (`circle.rs`, `line.rs`,
   `text.rs`, etc.); the shared declaration macro is in `properties.rs`.
   `fields.rs` only re-exports those definitions for existing public paths.
   Triangle/Chevron have separate object files and share the `wedge.rs` helper.
   Marker types describe field identity, not typestate transitions. `PhantomData`
   stores no marker value or object reference; actual object IDs still take space.
   `scene.add(description)` stores the object once, registers driven fields,
   and returns typed handles. For example, `square.position_x()` is a property
   target, not the current numeric x value. `Scene` is an object collection and
   construction context, not the renderer, window, or timeline itself.
3. `scene.compile(animations)` lowers a list of `Animation` values into one
   core timeline, validates native relationships, bakes action targets, and
   initialises sampled state. `try_compile` returns construction errors
   instead of panicking. Animations are values, so reusable functions and
   components return them; there is no merge API for compiled scenes yet.
   See [the authoring redesign](authoring-redesign.md) for the representation.
4. Sampling applies the core timeline, then evaluates the bindings whose
   inputs may have changed, in the prepared dependency order.
5. The host invokes a separate `vello_motiongfx::Renderer`, which reads sampled
   graphics and emits Vello scene commands. Its caches initialize lazily on
   first rendering and remain outside sampled/baked subject state.
   Geometry/text caches can reuse prepared data, but this is not automatic
   retained-layer rendering or partial-screen invalidation.

Timeline baking resolves relative targets and action data. It does **not** bake
every frame of arbitrary Rust procedural code. For seekable effects, calculate
state from sampled inputs/time. A simulation that accumulates `dt` needs its own
reset/replay/checkpoint strategy; neither callbacks nor signals supply that.

Object construction retains fixed/signal/computed builder setters. Typed getters
such as `position()`, `position_x()`, `radius()` and `fill()` return targets, not
sampled values. `.to(point)` accepts a typed Point (or tuple conversion);
`.to_xy(x, y)` is coordinate convenience. Relative helpers `shift`, `scale_by`
and `rotate` describe changes using action-start values. `fill_to` animates;
`set_fill` and `set_content` are immediate scheduled replacements. There is no
`.instant()` API. Generic `.set(value)` is still available on targets.
A shared `.spring(settings)` modifier works for absolute, relative, signal and
selection changes. Explicit tween timing/easing conflict with spring timing;
mixing them panics at the call site, without adding playback checks.
Per-property spring helpers and explicit field-selector closures are unnecessary.

## Signals and computed values: current contract

```rust
let progress = signal(0.0);
let radius = signal(80.0);
let position = computed!((progress, radius) => {
    let angle = progress * std::f64::consts::TAU;
    return (radius * angle.cos(), radius * angle.sin());
});
scene.add(Circle::new(&radius).position(&position));
scene.add(Circle::new(radius.map(|r| r * 0.1)).position(&position));
```

- `signal(initial)` and `Signal::new(initial)` describe writable timeline state.
- `computed(inputs, calculation)` and `Computed::new(...)` describe read-only
  calculations. `.map(...)` is the single-source form and also works on typed
  object properties. Computed values are not directly animatable; animate inputs.
- `computed!((a, b) => expression)` expands to
  `computed((&a, &b), move |(a, b)| expression)`. It always generates `move`.
  Blocks with explicit `return` also work. Listed inputs become sampled values;
  other variables are ordinary captures. Dependencies are not inferred from reads.
- Descriptions are standalone so helpers can build reactive objects without
  `&mut Scene`, including inline inside `scene.add(...)`.
- Descriptions use shared ownership, not `Copy` scene-local IDs. Borrowing into
  a retaining builder clones a shared handle; moving transfers that handle.
  Neither duplicates the calculation. This is setup work, not a per-frame clone.
- Registration prepares dense scene-local signal slots and computed indices.
  A shared computed description registers once per scene; standalone dependencies
  remain alive even if the original local handles are dropped. Independent scenes
  have independent sampled values/caches. Captured interior-mutable state inside
  user closures can still be shared; calculations should be pure.
- Descriptions used only by timeline actions are registered too. Unused standalone
  computed descriptions do not evaluate.

This replaced the older scene-owned `scene.signal`/`scene.computed` API. The old
small `Copy` IDs were cheaper to construct/reuse. The new representation adds
description allocations, reference-count operations and registration maps for
composability. Playback uses prepared sources, not description traversal. Compile
time, binary size and timing differences have not been benchmarked.

### Evaluation and rendering are separate

Each binding, `computed` value and `bind_mut` declares what it reads. When the
plan is prepared, it records which timeline sequences each node reads (grouped
by subject and source type, so this is one lookup per sequence, not a scan of
every pair), which nodes read the sample time (`scene.time()` and
simulations), and which nodes read each other. On each sample it marks the
nodes reading a sequence the timeline just wrote (`Timeline::queued_keys`, the
same set it sampled, including seeks and track changes), plus the time readers
if the time moved, and walks the prepared order running only marked nodes. A
marked node first compares its input with the last one and skips if it is the
same. A `computed` value that recalculates also compares its result with the
cached one, as Leptos's memos do, so a rounded, clamped or thresholded value
that comes out the same wakes nothing downstream. Only a node whose output may
have changed marks the nodes reading it. A `bind_mut` is marked one of two
ways: by its source, when it compares its input like any node, or by something
else writing one of its targets, when it writes again regardless to keep the
last word. The first sample after preparing runs everything.

So an idle binding costs nothing per sample, and an animating one costs the
timeline write plus its closure and write. What remains per sample is a lookup
for each written sequence of a watched subject type, a check of the time, and a
pass over the order's marks. This is declared-dependency tracking, not
automatic subscriptions, lazy demand-driven evaluation, or a general effects
system. Only the thin timeline conveniences lower away to ordinary MotionGfx
actions; do not call the signal system zero-cost. Large inputs can make
cloning/comparison expensive.

The timeline also skips a sequence while the playhead stays inside a gap
between two of its clips: the value written on entering the gap still holds,
so nothing is written and nothing reading it is marked. Without that, every
sequence whose clips span the current time was rewritten each sample, so
"written" meant little for signals animated in many short bursts.

Measured on the sorting example (64 bars, each fill a `computed!` of two
signals animated in short flashes), sampling the whole scene at 60 fps, release
build, best of 5: 35.4–35.9 µs per frame with the old evaluator, 37.8–38.6 µs
with change tracking alone (every signal sequence was still rewritten), and
6.6–6.7 µs with gaps skipped too (18.0 µs before the fills used signals at
all). The 2D authoring benchmark's playback, A/B against the same code without
these changes and run four times, moved between −6% and +7% per case, within
this machine's noise.

Comparing `computed` results and `bind_mut` inputs, measured pinned to the
performance cores in one session: a stepped `computed` (`floor(t * 10)`)
feeding 1,000 `bind_mut`s went from 7.6–7.8 µs to 0.53 µs per frame, and the
sorting example stayed at 6.7–7.0 µs. The authoring bench's playback, whose
bound circles follow a smoothly changing value so nothing is skipped and each
recalculation pays one extra comparison, moved 0% to +3%.

For a circle driven by a position signal, the binding checks that input and
updates stored position only on change. Rendering subsequently reads the stored
circle and emits drawing commands whenever the host renders a frame. Skipping
a calculation does not omit that circle from the picture.

## Which API to use

Reusable appearance settings use typed partial styles. `TextStyle` currently
contains optional typed inputs for font family/size/weight/style, spacing, wrapping, alignment, fill and opacity;
content/transforms/parent attachment remain object-specific. `Object::style`
accepts a compatible `ApplyStyle<T>` implementation and applies only configured
fields through the existing object setters. Later styles/setters win, including
replacing obsolete source wiring. Style/object setters are generated from the
same property declaration. Styles are construction-time presets, not scene
objects or retained CSS rules, and add no style-resolution pass in playback.
Signals/computed inputs retain their normal sharing/evaluation semantics.
Changing a preset afterward does not update existing objects; animate shared
inputs for coordinated changes. Only `TextStyle` is implemented so far; add
other concrete style families when needed, keeping unsupported properties off
their APIs. This does not introduce selectors, inheritance or a layout engine.

Line tips reuse ordinary circle/triangle/chevron descriptions. `scene.add(line)`
registers the tips automatically in shared shape pools, without independent
draw entries. The line renders them at its endpoints, handles orientation and
shaft trimming, and multiplies their opacity by its own. Typed `LineTipHandle`
exposes shape-specific properties (`radius` versus `angle`/`size`) without
runtime variant checks in authoring code. Standalone and attached shapes use
the same definitions/renderers. No ECS or general hierarchy is introduced.
Vello circle and wedge renderers expose internal painting entry points
with explicit parent transform and inherited opacity. Standalone calls pass
opacity `1.0`; endpoint attachments pass the line opacity. Each renderer
multiplies its own opacity exactly once. Triangle also receives its cached
geometry; the shared wedge painting implementation avoids duplicating triangle/chevron
fill/stroke logic. No default-opacity wrappers or general render-context
framework are needed for these arguments.
See the [tip API](../crates/motiongfx_2d/README.md#inline-endpoint-shapes)
for geometry, short-line fitting and current attachment limitations.

Rectangles have an `anchor`: the point their position marks and they rotate
and scale about, as a fraction of their half-size (y up), centre by default.
It combines placement and pivot, as Motion Canvas's `offset`, After Effects'
anchor point and Unity's `pivot` do, because the common case (a bar growing
from its baseline) needs both. Fractions rather than canvas units keep an edge
anchor on that edge while the size animates. It is an ordinary animatable,
signal-capable field; changing it with the position fixed moves the rectangle,
and no helper compensates because the size and rotation it would need are
only known when sampling. `Rectangle::local_rect` owns the anchored geometry so
renderers only draw it, and `relative_to` children hang off the anchor point.
It is on rectangles only: lines and paths have no box to take fractions of, so
it is not part of `Transform`. Pivoting about an outside point uses a parent.

Lines, paths, triangles, chevrons and rectangles expose typed `line_cap` (`Cap`)
and `line_join` (`Join`)
properties with round defaults, reactive inputs and discrete timeline changes.
These configure Kurbo stroke styles; they do not round filled geometry.
Triangle `.radius(...)` rounds its actual filled/stroked boundary, independently
of stroke joins. Open chevrons round their stroke with caps/joins and have no
geometry radius. Keep that distinction when extending shapes.
Full circles expose neither cap nor join settings: their outline is closed and
smooth. Stroke paint and optional cap/join builder code are shared separately.
Future arc/dash features can opt into applicable controls without adding unused
state now. Triangle already has filled, closed polygon geometry; it is not
implemented as a thick chevron stroke. Both retain common vertex/style/
registration code, while [rounded_triangle.rs](../crates/motiongfx_2d/src/rounded_triangle.rs)
builds circular fillets in a closed path for Triangle only. Effective radius is
clamped to the inradius, leaving the requested typed property unchanged.
The nominal apex anchor is preserved: rounding clips back the visible point.
Per-triangle geometry caches in the Vello `Renderer` sit outside cloned/sampled Triangle
state and rebuild on size/angle/radius changes during rendering. Tip trimming
uses the cached curved boundary. No per-frame heap path allocation is needed;
ordinary cache checks, bounds/transforms, and Vello recording still take work.

- Use ordinary timeline properties for authored changes: `.to`, `.offset`,
  `.multiply`, `.set`, `.to_with`, `.over`, `.ease`, `.spring`, combined with
  `chain`, `all`, `any`, `flow`, `wait` and `marker`. Constructing an animation alone
  does not schedule it (it is `must_use`); passing it to `scene.compile` does.
- `flow(interval, [..])` starts children `interval` apart and delegates to core
  `ord_flow`, reserving the latest child end. Children can be changes or nested
  groups, built from any iterator.
- Use reactive builder inputs for relationships between objects/values. Share
  common calculations through computed descriptions instead of repeating work.
- `scene.bind` is advanced explicit wiring after object creation. Builder inputs
  normally register equivalent relationships automatically.
- `scene.bind_mut` is `bind` with mutable access to whole objects, for
  changing several objects from one calculation or one object in place (such
  as rewriting a path's geometry). It runs when its source changes or
  something else writes one of its targets, so it has the last word on them,
  and bindings reading its targets run after it. Keep it for those cases, not
  as the default for every animated property.
- What a value depends on decides the API: authored time is the timeline;
  other values right now are signals, computed values, builder inputs, `bind`
  and `bind_mut`; its own past is `simulate`. One-off happenings such as sounds
  are timeline `event`s that hosts act on during playback; nothing runs side
  effects while sampling, which also happens for seeks, tests and exports.
- `scene.simulate` / `simulate_with` are sources that advance in fixed steps
  from their previous state (collisions, forces, feedback). They run once at
  `compile` and are looked up by time, so seeking stays exact. See
  [simulations.md](simulations.md); deterministic particles are planned in
  [particles.md](particles.md).
- The callback `Sequence` API and its `animate!` macro were removed once every
  example and test used `scene.compile`; `computed!` remains.

Rust checks source/output/property types. Preparation checks binding cycles,
missing properties, overlapping binding writers and timeline writes to bound
fields. A `bind_mut` may share objects with bindings and other `bind_mut`s,
which it then writes after; because it writes whole objects, a binding that it
depends on reads its targets as they were before it wrote, instead of forming a
cycle.
Ordinary graphics handles do not enforce scene identity; mixing handles between
scenes is unsupported and may alias a valid local ID. Purity, numerical validity,
and simulation determinism are not guaranteed by the type system.

## Analytical springs

Springs are core actions, with analytical response maths in motiongfx_interp
and thin property/signal `.to(value).spring(settings)` descriptions in authoring. Physical, visual and timed
settings are distinct configuration types. Neither spring change exposes over/ease.
They start at rest, use finite normalized settling tolerances, and reconstruct
from absolute elapsed time. The public SpringCurve sampler also drives graphs.
See [spring contracts](springs.md) for exact units, defaults, validation and examples.
Visual duration uses Motion's frequency/bounce calibration, including overdamped
compensation; it is a pacing control, not the first 95% crossing or a fixed end.
Timed mode calibrates frequency to fit conservative rest envelopes within an
exact requested duration; the existing sampler and timeline need no new mode.

Springs reserve their full settling duration (the exact requested time for timed
mode) using existing track ordering. `settle_duration()` exposes that reserved time
(fallible, since it prepares the curve); `TimedSpring::duration()` is the
infallible exact value, letting non-springable siblings such as colors tween in
step via `.over(spring.duration())`. Authoring springs any interpolated
property; only discrete properties are refused, when compiling.
There is no arrival-based continuation or spring-specific overlap policy.
Track compilation is unchanged; authoring turns the clips core would drop for
overlapping a later write into `compile` errors with both lines.
Invalid spring settings are reported by Scene::try_compile as
CompileError::Animation, with the animation's file and line.

Rest delta/speed are normalized (0.001 and 0.01/s), deliberately independent of
start-to-target distance so existing ordering can schedule before target baking.
They are not Motion-style absolute pixel/property-unit thresholds. Preparation
uses a conservative energy bound for physical/visual settlement, and analytical
tail envelopes for timed calibration. There is no initial velocity,
live settings change, or interruption support.
Future takeovers must preserve the earlier clip until takeover and seed the next
motion from its sampled position/velocity without changing the original time scale.
The spring_curves example compares physical, visual and timed spring-modified actions;
settings interpolate through signals BETWEEN runs. Its presets expose ordinary
Spring builders, with automatic direction alternation, shared time-axis fitting,
parameter/settling readouts and synchronized response markers. Raw Spring
descriptions support bounded same-mode interpolation and diagnostic Display;
this does not change the fixed prepared coefficients of active actions.

## Reference examples and current limits

See the [example guide](../examples/vello_winit_example/README.md) for commands.
Useful comparison cases:

- `procedural_wave_low_level`: custom `VelloDemo` integration and direct Vello
  drawing without a MotionGfx timeline. A host-extension reference, not the
  recommended animation-authoring pattern. `timeline_wave` is the orchestrated
  MotionGfx wave comparison.
- `moving_square`: direct native property changes.
- `insertion_sort`: ordinary Rust procedural construction/sorting builds a
  timeline, anchored labels follow bars, and one `all([..])` per pass moves
  them together.
- `commit_graph`: a reusable `Commit` component whose methods return
  animations, parallel pulses and a `flow` stagger.
- `sine_unit_circle`: fixed trace geometry revealed by a shared progress signal.
  `.map`/`computed!` positions feed object builders directly, reconstructing the
  orbit/connector on seeks without explicit `scene.bind` calls.
- `bezier_explainer`: standalone signals, shared computed points, exact dynamic
  quadratic geometry, reusable grid construction, and timed chapter changes.
- `radius_area`: inline `.map` inputs and computed text/geometry from one signal.
- `lissajous`: precomputed procedural paths, per-curve range signals, reactive
  reference dots and nested diagonal timing on one timeline. Nested `flow`
  groups delegate to core `ord_flow`, alongside `all` (`ord_all`) and `chain`
  (`ord_chain`), without manual delay calculations. A `marker("loop")` after
  the intro configures `LoopFrom`; no separate timeline
  or custom renderer is used for either chapter. It uses standard canvas fitting
  on resize. Headless seek/timing regressions are in the example support folder.
- `camera_zoom`: camera transforms without changing the underlying objects.

Native `Text` now uses Parley shaping and Vello glyph runs. The independent
`motiongfx_text` crate retains source/cluster mappings and advance/line bounds;
`motiongfx_2d` owns portable text/selection state; Vello owns shared font contexts
and per-object layout and paint-span caches. Default output uses embedded Fira Sans, not OS
font discovery or the old hardcoded lettering. Additional fonts can be registered
on the renderer and supplied to the host with `Window::renderer`.

Layout keys contain content and font/layout inputs plus font registration revision.
Paint, opacity and transforms do not reshape. Paint spans map contiguous glyph
ranges to selection IDs once per layout/selection-structure change; per-frame
paint reads do not redo glyph/source overlap matching. These associations are
independent of transforms and can survive future glyph geometry changes when
source identity is preserved. Glyph-correspondence changes must invalidate them.
Invalid numeric styles produce no glyphs. Static font faces do not offer continuous weight morphing; variable-font
coordinates are preserved when supplied by Parley. No benchmark is claimed.

The current cache keys are compared during rendering, not explicitly invalidated
by setters. Selection ranges are fixed in the public authoring API; their
per-render comparison is conservative, including for renderer reuse across
animations. This implementation remains unchanged for now. See
[text-caching.md](text-caching.md) for exactly what is checked, why shaping and
paint associations are cached, and what to revisit with future dynamic effects.

Typed literal/byte-range selections animate fill/opacity without independent draw
nodes. `text.select("word")`, `select_nth` and `select_range` now return deferred
descriptions; only used selections register while lowering actions. Lowering
receives a CompileContext with model access, signal registration and
construction-only typed memoization. These registration maps drop before playback.
Missing/invalid queries report CompileError::Animation with query/source/call location
before baking or sampling. Equivalent resolved ranges reuse the same target;
distinct overlaps use registration order (first scheduled use), later wins.
Selection descriptions share one query allocation across their paint fields;
prepared actions retain only ordinary property IDs/accessors, not descriptors.
Selections use the initial fixed source snapshot, validate UTF-8 boundaries,
and expand to whole clusters. Changed source disables them; backward seeking to
the same source restores them. Selection fill is independently owned, initialized
from the text fill. No automatic edit remapping,
per-part transforms, text matching, write-in, code highlighting or Typst adapter
is implemented. These are future extensions, not reasons to add another timeline.
Legacy immediate 5x7 lettering remains for preview chrome and two low-level demos;
it is not an authored font mode. Full layout, 3D and a general effect stack remain
outside the current scope.

Shape morphing, animatable gradients/shadows/blur and
richer text/code transitions have been discussed as future capabilities. Their
designs are not locked in by this document. Keep graphics effects associated
with their owning object where appropriate, and preserve the core timeline
instead of introducing a separate scheduler without a demonstrated need.

## Sound events

Sounds are part of the animation, not a host setting; see
[audio.md](audio.md). Authoring has one generic leaf,
`event(payload, length)`, for things that aren't property changes. Markers
and events both lower to core **cues** (numbered points that core's
combinators shift exactly like clips), so authoring never recomputes
offsets. Core has no named markers; authoring maps each cue to its marker
name or event payload after compiling, and `compiled.events::<E>()` reads
events by payload type. `motiongfx_audio` makes the payloads (`note`,
`play`), collects them with `Score::new(&compiled)`, renders offline, and
plays live from the host clock. Sampling ignores events, so seeking and
visual sampling are unchanged. Sound plays during forward playback only.

## Future options, not implemented commitments

### Interrupting animations across aliasing paths

`.interrupt()` cuts a running animation of the same property (implemented).
Cutting only the `x` of a running `position` animation, and springs that keep
their speed when cut, are designed in [interruption.md](interruption.md).

### More fine-grained updates and idle rendering

The current `signal`/`.map`/`computed!` authoring shape can be preserved while
improving evaluation internally. Profile first. Potential improvements:

1. Dirty propagation that skips unaffected nodes is implemented (see
   "Evaluation and rendering are separate"). Every write path participates:
   timeline sequences, the sample time, bindings and `bind_mut` targets.
2. Separate visual invalidation could let the host avoid unnecessary redraws
   during pauses/static holds. Resize, exposure, controls, camera, interaction
   and background changes still need appropriate redraws.
3. Renderer command/layer caching is another optimisation, separate from signal
   caching. Unchanged objects still have to appear in each resulting picture.

Large graphs and interactive/idle scenes may benefit most. Continuous animation
can change most inputs each frame; tracking adds bookkeeping and is not
automatically faster. No automatic effects/subscriptions or partial redraw
system is currently promised. Adding arbitrary side effects would also require
a lifecycle/seeking policy, not just another signal constructor.

### Scripting, hot reload and documents

Future language adapters should reuse core timeline operations and graphics
capabilities where practical; no Rhai/JS/TypeScript runtime is implemented.
Language choice and a script execution/data model remain open.

A script can execute loops/helpers once to construct a native timeline. That
does not make arbitrary script functions run as compiled Rust per frame. Native
per-sample procedural calculations would need representable operations/data,
code generation, or explicitly accepted script callbacks/interpreter costs.
The present native dependency plan stores Rust closures; it is not a serializable
node-graph language that scripts can automatically target without more work.

Evaluate `motiongfx_scene`'s existing serialization, registries and compiler
before inventing a second document/timeline format. It does not currently store
the authoring closure graph, and is not required just to shorten native code.
Do not claim a future node editor or hot reload is already supported.

Native authoring/rendering separation leaves room for alternate hosts/backends.
A browser/Wasm host and language bindings still need implementation and testing;
the current Winit host is the native reference, not proof of browser parity.

## Keeping this record useful

Document API specifics in crate READMEs and source docs, with working examples.
Update this note when a boundary or decision changes. Label proposed features
as proposals and record actual limitations; avoid treating old conversational
sketches, unmeasured performance estimates, or this future-options section as
implemented functionality.
