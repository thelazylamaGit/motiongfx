# MotionGfx Authoring

See [architecture and direction](../../docs/architecture.md) for crate
boundaries, design rationale, limitations and future options.

Native, typed setup conveniences over existing MotionGfx. Animations are
values: build them anywhere, return them from functions, store, clone and
reuse them, then pass a list to `scene.compile(..)`, which lowers them to
ordinary core actions. Playback is plain core sampling; there is no separate
evaluator.

```rust,ignore
let mut animation = scene.compile([
    square.position_x().to(100.0).over(ms(300)),
    wait(ms(200)),
    all([
        square.rotate(0.4).over(ms(500)),
        chain([
            square.fill_to(ORANGE).over(ms(200)),
            square.scale_by(0.5).over(ms(300)),
        ]),
    ]),
    flow(ms(100), bars.iter().map(|bar| bar.shift_y(40.0))),
    marker("end"),
]);
```

## Animations

`Animation<M>` is one change, group, wait or marker (`M` is the scene's
model; `motiongfx_2d` exports `Animation` with it filled in). The list passed
to `compile` runs in order, like `chain`.

- Changes come from properties, signals and object handles: `.to(value)`,
  `.set(value)` (instant), `.offset(delta)`, `.multiply(factor)` and
  `.to_with(|start| target)`, whose target is calculated from the value at
  the change's start. Relative targets are resolved once while baking, never
  per frame.
- `chain([..])` runs children one after another; `all([..])` starts them
  together and lasts as long as the longest; `flow(interval, [..])` starts
  them `interval` apart. Groups accept any iterator, so
  `all(items.iter().map(|item| item.fade_in()))` builds one per item.
- `wait(duration)` pauses. Inside `all` it doesn't delay siblings; use
  `.delay(duration)` on an animation for that.
- `marker("name")` names a point in time, read back with
  `compiled.marker("name")` (for example a loop point). Names must be unique.

`.over(duration)` and `.ease(function)` time one change. Without them, the
scene defaults apply: one second and the normalized sigmoid `smooth` easing,
changeable with `scene.defaults(duration, ease)` at any point before
`compile`. Groups, waits and markers have no timing of their own, so timing
them panics at the caller's line.

`Animation` is marked `must_use`: building one without passing it to
`compile` or a group does nothing. Its `Debug` output shows the tree and the
file and line of each change.

Each change shares its description behind an `Arc`, so cloning an animation
is cheap; compiling costs one allocation per change. See the
[measurements](../../docs/authoring-redesign.md#11-measurements).

## Custom steps

A model can add its own kinds of change by implementing `Step<M>` and
wrapping it with `Animation::from_step`. During `compile`, `Step::lower`
receives a `SceneContext<M>`, which gives model access, construction-only
caches and `report(error)`; most steps resolve their target, then call
`lower_change`. Text selections in `motiongfx_2d` work this way.

## Fields and targets

`Property<I, S, T>` stores a subject ID, a typed field path and how the
field interpolates: smoothly, or discretely (jumping to the target at the
end, for text content and caps/joins). Built-in model fields instead expose
`Target<I, S, T, F>`: a zero-sized `Field` marker identifies the accessor, so
each target stores only its subject ID and expands to a `Property` when
compiled. Runtime-selected paths still use `Property`; compact targets
convert with `.into()` or `.property()`.

`PhantomData<fn() -> (S, T, F)>` records the subject/value/field types without
owning values of those types or storing runtime accessor pointers. Model crates
keep marker declarations beside the owning objects; their shared macros only
remove repetitive definitions.

This crate has no Vello, Winit, or scene-document dependency. Custom Rust
subject types can expose their own typed `Property` fields.

## Shared scene lifecycle

`Scene<M>` owns an application-selected model `M`, one native binding plan,
and the ordered native update callbacks. `World<M>` wraps the model with
scene-local signal storage and sampled time. Models implement the core
`SubjectSource<ModelId, ModelState> for World<M>` using model-owned typed IDs.
There is no rendering dependency, closed list of shape kinds, or per-sample
type registry in this shared layer.

`SceneObject<M>::insert_into` registers state and returns typed handles. A
handle is an index into its scene; use it only with that scene and its
compiled result. `ObjectDefinition<T, M>` supplies reusable
descriptor/property wiring; model crates may wrap it to expose their own
inherent builders and input conversions. `Input<M, T>` prepares
fixed/signal/computed/property inputs. `SampleTime` provides the clamped
sample time as an explicit source through `scene.time()`.

`scene.try_compile(animations)` runs preparation hooks, lowers the animations
into the core registry and timeline, validates the binding plan, bakes
relative targets, and samples time zero. `compile` is the panicking
convenience: animation mistakes are bugs in the source, so by default it
panics with every problem found, each with its file and line. Use
`try_compile` to handle them instead (tests, tools). `CompiledScene<M>::sample(time)`
applies the core timeline, evaluates bindings, then runs direct callbacks in
registration order. `model()` exposes a read-only sampled model; `state()`
also exposes shared time and registered signal inspection. Renderers consume
these views separately and own their own caches/resources.

`UpdateTargets<M>` prepares a native callback with model-specific safe target
borrowing. The optional `UpdateStorage`/`UpdateHandle` contracts implement
array/tuple target collections without a renderer-specific collection engine.
Direct callbacks run on every sample, have no dependency/writer sorting, and
must assign from sampled inputs for arbitrary-time seeking. Mutable models
and callbacks remain native Rust; this does not add a scripting runtime.
Invalid update sources or targets panic at the `scene.update(..)` call.

`prepare_with<Key>(...)` is an integration hook for final configuration such
as last-setter-wins backgrounds. It runs during construction and is keyed by
type only there. It is not a per-frame system registry.

See [headless shared model tests](tests/shared_scene.rs) and the
[portable 2D model](../motiongfx_2d/README.md) for complete adapters.
`BindingPlan<W>` remains usable without this scene lifecycle.

## Errors

`try_compile` returns `SceneError::Binding(BindingError)` or
`SceneError::Animation(Vec<AnimationError>)`. Each `AnimationError` has the
file and line of the animation and a `kind`:

- `Spring(SpringError)`: invalid spring settings;
- `DiscreteSpring`: a spring on a discrete property;
- `Target(..)`: a target that could not be resolved, such as a missing text
  selection (downcast it to inspect the concrete error);
- `DuplicateMarker`: a marker name used twice, with both locations.

Both enums are `#[non_exhaustive]`, so matches need a `_` arm.

## Spring changes

`.spring(settings)` animates any interpolated property, signal or relative
helper (`square.shift(100.0, 0.0).spring(settings)`) with a core analytical
spring instead of a tween; the spring decides the duration. It cannot be
combined with `.over()`/`.ease()` or `.set()`: doing so panics at that call.
Scene defaults are simply replaced by the spring's timing. Discrete
properties cannot spring; `compile` reports that.

`Spring::physical()` configures stiffness/damping/mass; `Spring::visual(duration)`
configures visual timing/bounce. Both support normalized rest tolerances.
`Spring::timed(duration).bounce(...)` fits settlement into the requested time:

```rust,ignore
ball.position_x().to(200.0).spring(Spring::timed(ms(500)).bounce(0.4))
```

Springs reserve their full settling duration (exactly the requested time for
timed mode). `TimedSpring::duration()` returns that time infallibly, so sibling
tweens can match it; every spring setting also has a fallible
`settle_duration()`, and `VisualSpring::visual_duration()` returns its pace:

```rust,ignore
let spring = Spring::timed(ms(650)).bounce(0.65);
flow(ms(300), [
    square.shift(-80.0, 0.0).spring(spring),
    square.fill_to(ORANGE).over(spring.duration()),
])
```

Overshoot is the interpolation's own extrapolation: colour channels briefly
leave 0–1 and rendering saturates them. Existing core overlap/conflict rules
apply unchanged; there is no spring-specific interruption policy. Settings
are fixed per action; no live parameter changes or velocity-preserving
interruption is implied. See [spring usage and limitations](../../docs/springs.md).

## Native derived relationships and validation

`BindingPlan<W>` optionally calculates properties after timeline sampling.
`bind(destination, source, function)` stores an ordinary native Rust closure;
`BindingPlan::computed(source, function)` returns a typed, copyable `ComputedSlot<T>` handle for
sharing one cached calculation. Sources can be properties, derived handles,
or tuples of two to five sources. Multiple source values arrive as a tuple.
Input/output types are checked by Rust; input values implement `PartialEq`
so unchanged inputs skip calculation and writes. Sources and cached outputs
are cloned by value; choose cheap/shared types for large data.

Call `prepare(world, timeline)` once before evaluation. It rejects missing
properties, foreign derived handles, duplicate/overlapping binding writers,
timeline actions on bound destinations, and cyclic dependencies. Validation
uses compiled action metadata. Whole fields overlap their nested axes; sibling
axes are independent. Bound properties can feed other bindings, even when
consumers were registered before producers.

Call `evaluate(world)` after each sample. A fixed dependency order is reused;
there is no per-frame graph sorting. Shared derived outputs reuse their storage
after initialization. Native closures still have execution/dispatch costs, and
input reads, comparisons and destination writes are additional work. Evaluation
does not use elapsed frame time; deterministic functions reconstruct their
results during backward seeking. Interior mutable state, wall-clock reads,
randomness and numerical validity are not statically checked.

This is optional native binding support, not automatic dependency tracking or
a scripting interpreter. The shared `Scene<M>` owns the plan and finalizes it
during `try_compile` (returns `SceneError`) or `compile` (panics).

## Standalone sources

`signal(initial)` / `Signal::new(initial)` and
`computed(inputs, function)` / `Computed::new(inputs, function)` construct
scene-independent descriptions. `signal.map(function)` and
`computed.map(function)` describe single-source calculations. `Property::map`
also supports sampled native properties. These retain owned dependencies; a
helper can return a description without retaining references to its arguments.
`computed!` avoids repeating dependency names.

`Source<W>::register(world, plan)` prepares a description for one world and
deduplicates its computed nodes. Integrations implement `SignalWorld` with
`SignalValues` storage and `SubjectSource<SignalId, T>` access. A scene
registers signals automatically, including ones used only in animations.

Descriptions are cloneable shared ownership, not `Copy` runtime IDs. Each
world has independent sampled values and caches. Registration maps and source
descriptions are not traversed during sampling; evaluation retains the native
source reads, comparisons and closure dispatch described above. This is explicit
dependency wiring, not automatic tracking, effects, or an interpreter.
