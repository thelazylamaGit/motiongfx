# Native authoring architecture and direction

This note records the current native authoring work as of 2026-11-12. It is a
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

## Crate responsibilities

| Crate | Responsibility and boundary |
| --- | --- |
| [`motiongfx`](../crates/motiongfx/README.md) | Typed actions, timelines, ordering, baking and sampling; independent of graphics/windowing. |
| [`motiongfx_interp`](../crates/motiongfx_interp/README.md) | Interpolation support. |
| [`motiongfx_authoring`](../crates/motiongfx_authoring/README.md) | Typed property/change descriptions, sequence conveniences, optional `animate!`, standalone signal/computed descriptions and native dependency evaluation. No Vello or Winit dependency. |
| [`peniko_motiongfx`](../crates/peniko_motiongfx/README.md) | Geometry/color interpolation and path-related integration; not a window host or complete scene renderer. |
| [`vello_motiongfx`](../crates/vello_motiongfx/README.md) | Graphics state, typed shape/text/camera handles, source-aware object builders, native updates, render caches and Vello drawing. Re-exports/specialises authoring facilities, including `computed!`. |
| [`vello_winit_motiongfx`](../crates/vello_winit_motiongfx/README.md) | Native window/GPU setup, resize/DPI handling, playback clock, play/pause and scrubbing controls. These are shared host facilities, not required scene objects. |
| [`motiongfx_scene`](../crates/motiongfx_scene/README.md) | Serializable scene/timeline documents, named registries and compilation into core actions. Distinct from `vello_motiongfx::Scene`; not currently a dependency of native authoring. |
| [`bevy_motiongfx`](../crates/bevy_motiongfx/README.md) | Separate Bevy integration. The Vello/Winit authoring path does not require Bevy. |
| [`velyst_motiongfx`](../crates/velyst_motiongfx/README.md) | Existing Bevy/Velyst/Typst integration, currently excluded from the workspace pending dependency compatibility. Not yet the text implementation for this Vello host. |

Keep these existing boundaries. There is currently no separate signals crate
or language-binding crate; split further when a concrete dependency or reuse
requirement warrants it, rather than creating empty architectural layers.

Implementation entry points:

- [Standalone source descriptions](../crates/motiongfx_authoring/src/source.rs),
  [signals and storage](../crates/motiongfx_authoring/src/signals.rs), and
  [prepared dependency evaluation](../crates/motiongfx_authoring/src/bindings.rs).
- [Object descriptors](../crates/vello_motiongfx/src/object.rs),
  [reactive input registration](../crates/vello_motiongfx/src/input.rs), and
  [scene finalisation/sampling/rendering](../crates/vello_motiongfx/src/lib.rs).
- [Direct update callbacks](../crates/vello_motiongfx/src/updates.rs) and
  [native host](../crates/vello_winit_motiongfx/src/lib.rs).

## Construction, sampling, rendering

1. Constructors such as `Rectangle::new` return `Object<T>` descriptions.
   Fixed styles/defaults are initial state. Reactive setters retain typed inputs.
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
5. Rendering reads stored graphics state and emits Vello scene commands.
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

## Reference examples and current limits

See the [example guide](../examples/vello_winit_example/README.md) for commands.
Useful comparison cases:

- `moving_square`: direct native property changes.
- `insertion_sort`: ordinary Rust procedural construction/sorting builds a
  timeline, anchored labels follow bars, and `play_all` moves them together.
- `commit_graph`: reusable circles/rings/paths, parallel pulse animations, stagger.
- `sine_unit_circle`: fixed trace geometry revealed on the timeline, with native
  relationships reconstructing the orbit/connector on seeks.
- `bezier_explainer`: standalone signals, shared computed points, exact dynamic
  quadratic geometry, reusable grid construction, and timed chapter changes.
- `radius_area`: inline `.map` inputs and computed text/geometry from one signal.
- `camera_zoom`: camera transforms without changing the underlying objects.

Native `Text` currently uses temporary vector lettering, not general font
shaping. Do not describe it as Parley/Typst support. Camera/parent transforms and
basic text are present; full layout, rich code/glyph transitions, 3D, and a
general effect stack are not established by this API.

Configurable springs, shape morphing, animatable gradients/shadows/blur and
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
