# Native authoring architecture and direction

This note records the current native authoring work as of 2026-11-13. It is a
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
| [`motiongfx_authoring`](../crates/motiongfx_authoring/README.md) | Shared `Scene<M>`/`Animation<M>` lifecycle, registration/property preparation, sequences, signals/computations and native binding/update execution. No graphics/window dependency. |
| [`motiongfx_2d`](../crates/motiongfx_2d/README.md) | Portable 2D objects/storage, geometry, styles, typed handles, attachments and camera semantics. Uses Kurbo/Peniko directly, not Vello/Winit. |
| [`peniko_motiongfx`](../crates/peniko_motiongfx/README.md) | Geometry/color interpolation and path-related integration; not a window host or complete scene renderer. |
| [`vello_motiongfx`](../crates/vello_motiongfx/README.md) | Vello drawing and renderer-owned caches. Consumes portable sampled models; compatibility re-exports do not implement a second model or lifecycle. |
| [`vello_winit_motiongfx`](../crates/vello_winit_motiongfx/README.md) | Native window/GPU setup, resize/DPI handling, playback clock, play/pause and scrubbing controls. These are shared host facilities, not required scene objects. |
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

Authoring owns `Scene<M>`, `Animation<M>`, and `World<M>`. World wraps an
application-selected model with signal storage and sampled time. Model crates
implement core `SubjectSource<ModelId, State> for World<M>` using their own IDs.
Shared code never enumerates graphics/3D types or invokes a renderer.

The 2D convenience aliases specialise that lifecycle to `Graphics`. Local
`Object<T, M>` and `Input<T, M>` wrappers add inherent builders and geometry
conversions over generic `ObjectDefinition` / `Input` preparation in authoring.
They satisfy Rust's inherent-implementation/orphan rules while keeping one
preparation implementation. Detached builders may need an explicit
`Object<Circle>` type when no scene provides inference.

`Model2d` exposes a contained Graphics store in composite models. Ordinary
2D objects and another model can share signals, computed relationships and one
core schedule. This is tested with actual 2D objects plus a small second model.
There is no 3D renderer or automatic Bevy adapter. Custom mixed callback targets
still require a safe model adapter; provided 2D array/tuple borrowing targets
Graphics. No ECS or general dynamic world registry is introduced.

2D-only helpers are extension traits imported by `motiongfx_2d::prelude::*`:
`Scene2dExt` for camera/background setup, `Animation2dExt` for sampled reads.
The inherent lifecycle methods (`add`, `animate`, `bind`, `update`, `sample`)
remain shared.

## Construction, sampling, rendering

1. Constructors such as `Rectangle::new` return `Object<T>` descriptions.
   Fixed styles/defaults are initial state. Reactive setters retain typed inputs.
   Adding endpoint shapes produces a typed `LineWithTips` construction wrapper.
   Its handle exposes ordinary line properties and the concrete shape handles.
2. `scene.add(description)` stores the object once, registers driven fields,
   and returns typed handles. For example, `square.position.x` is a property
   target, not the current numeric x value. `Scene` is an object collection and
   construction context, not the renderer, window, or timeline itself.
3. `scene.animate(|sequence| ...)` authors and compiles one timeline, validates
   native relationships, bakes action targets, and initialises sampled state.
   `try_animate` reports construction errors instead of panicking. Compose
   reusable functions within this process; there is no animation-merge API yet.
4. Sampling applies the core timeline, evaluates the prepared binding plan,
   then runs direct update callbacks in registration order.
5. The host invokes a separate `vello_motiongfx::Renderer`, which reads sampled
   graphics and emits Vello scene commands. Its caches initialize lazily on
   first rendering and remain outside sampled/baked subject state.
   Geometry/text caches can reuse prepared data, but this is not automatic
   retained-layer rendering or partial-screen invalidation.

Timeline baking resolves relative targets and action data. It does **not** bake
every frame of arbitrary Rust procedural code. For seekable effects, calculate
state from sampled inputs/time. A simulation that accumulates `dt` needs its own
reset/replay/checkpoint strategy; neither callbacks nor signals supply that.

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

The current evaluator visits every registered binding/computed node on each
sample, reads/clones its inputs, and compares them with previous inputs.
Unchanged inputs skip the calculation and destination write. Changed inputs run
native Rust closures; shared calculations have cached outputs. Dependency order
is prepared once, not sorted every frame.

This is sample-and-compare reactivity, not automatic subscriptions, lazy
demand-driven evaluation, or a general effects system. It has reads, comparisons,
cloning and closure dispatch costs. Only the thin timeline conveniences lower
away to ordinary MotionGfx actions; do not call the entire signal system
zero-cost. Large inputs can make cloning/comparison expensive.

For a circle driven by a position signal, the binding checks that input and
updates stored position only on change. Rendering subsequently reads the stored
circle and emits drawing commands whenever the host renders a frame. Skipping
a calculation does not omit that circle from the picture.

## Which API to use

Reusable appearance settings use typed partial styles. `TextStyle` currently
contains optional typed inputs for font size, alignment, fill and opacity;
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
  `.multiply`, `.set`, `.over`, `.ease`, waits, parallel groups, and stagger.
  Constructing a change alone does not schedule it; `sequence.play` does.
- Use reactive builder inputs for relationships between objects/values. Share
  common calculations through computed descriptions instead of repeating work.
- `scene.bind` is advanced explicit wiring after object creation. Builder inputs
  normally register equivalent relationships automatically.
- `scene.update` is a direct `FnMut` callback for grouped mutation or work that
  does not fit property relationships. It runs on **every** sample, after
  bindings, without change caching or dependency ordering between callbacks.
  Keep it for those cases, not as the default for every animated property.
- `animate!` remains optional. The main examples use plain sequence methods;
  do not migrate them back to that macro just for consistency. It is distinct
  from the small `computed!` dependency shorthand.

Rust checks source/output/property types. Preparation checks binding cycles,
missing properties, overlapping binding writers and timeline writes to bound
fields. Direct updates do not have equivalent writer/dependency validation.
Ordinary graphics handles do not enforce scene identity; mixing handles between
scenes is unsupported and may alias a valid local ID. Purity, numerical validity,
and simulation determinism are not guaranteed by the type system.

## Analytical springs

Springs are core actions, with analytical response maths in motiongfx_interp
and thin property/signal spring_to descriptions in authoring. Physical, visual and timed
settings are distinct configuration types. Neither spring change exposes over/ease.
They start at rest, use finite normalized settling tolerances, and reconstruct
from absolute elapsed time. The public SpringCurve sampler also drives graphs.
See [spring contracts](springs.md) for exact units, defaults, validation and examples.
Visual duration uses Motion's frequency/bounce calibration, including overdamped
compensation; it is a pacing control, not the first 95% crossing or a fixed end.
Timed mode calibrates frequency to fit conservative rest envelopes within an
exact requested duration; the existing sampler and timeline need no new mode.

Springs reserve their full settling duration (the exact requested time for timed
mode) using existing track ordering.
There is no arrival-based continuation or spring-specific overlap policy.
Track compilation and ordinary action conflict behavior remain unchanged.
Authoring Sequence::try_compile reports invalid spring settings; Scene::try_animate
returns SceneError, wrapping BindingError or SpringError.

Rest delta/speed are normalized (0.001 and 0.01/s), deliberately independent of
start-to-target distance so existing ordering can schedule before target baking.
They are not Motion-style absolute pixel/property-unit thresholds. Preparation
uses a conservative energy bound for physical/visual settlement, and analytical
tail envelopes for timed calibration. There is no initial velocity,
live settings change, or interruption support.
Future takeovers must preserve the earlier clip until takeover and seed the next
motion from its sampled position/velocity without changing the original time scale.
The spring_curves example compares physical, visual and timed spring_to actions;
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
  timeline, anchored labels follow bars, and `play_all` moves them together.
- `commit_graph`: reusable circles/rings/paths, parallel pulse animations, stagger.
- `sine_unit_circle`: fixed trace geometry revealed by a shared progress signal.
  `.map`/`computed!` positions feed object builders directly, reconstructing the
  orbit/connector on seeks without explicit `scene.bind` calls.
- `bezier_explainer`: standalone signals, shared computed points, exact dynamic
  quadratic geometry, reusable grid construction, and timed chapter changes.
- `radius_area`: inline `.map` inputs and computed text/geometry from one signal.
- `lissajous`: precomputed procedural paths, per-curve range signals, reactive
  reference dots and nested diagonal timing on one sequence. Scoped
  `sequence.staggered` groups delegate to core `ord_flow`, alongside `parallel`
  (`ord_all`) and `chain` (`ord_chain`), without manual delay calculations.
  The intro group returns its reserved duration to configure `LoopFrom`; no separate timeline
  or custom renderer is used for either chapter. It uses standard canvas fitting
  on resize. Headless seek/timing regressions are in the example support folder.
- `camera_zoom`: camera transforms without changing the underlying objects.

Native `Text` currently uses temporary vector lettering, not general font
shaping. Do not describe it as Parley/Typst support. Camera/parent transforms and
basic text are present; full layout, rich code/glyph transitions, 3D, and a
general effect stack are not established by this API.

Shape morphing, animatable gradients/shadows/blur and
richer text/code transitions have been discussed as future capabilities. Their
designs are not locked in by this document. Keep graphics effects associated
with their owning object where appropriate, and preserve the core timeline
instead of introducing a separate scheduler without a demonstrated need.

## Future options, not implemented commitments

### More fine-grained updates and idle rendering

The current `signal`/`.map`/`computed!` authoring shape can be preserved while
improving evaluation internally. Profile first. Potential improvements:

1. Version counters/dirty propagation could skip checks of unaffected nodes.
   Every supported write path, including timeline sampling, must participate.
   Direct mutable callbacks require particular care; incorrect invalidation
   would leave stale state. Preserve arbitrary forward/backward seeking.
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
