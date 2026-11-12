# MotionGfx Authoring

See [architecture and direction](../../docs/architecture.md) for crate
boundaries, design rationale, limitations and future options.

Native, typed setup conveniences over existing MotionGfx. `Property` stores
a subject ID, typed field path and interpolation function. `to`, `offset`,
`multiply` and `map_target` create descriptions; `Sequence::play` lowers them
to normal actions. Relative targets use the action-start value during baking.
These timeline descriptions add no separate evaluator during playback.

`AnimationDefaults::default()` defines the library-wide authoring defaults:
one second and normalized sigmoid `smooth` easing. Scenes need no defaults
declaration. `over(duration)` and `ease(function)` override individual changes;
`Sequence::defaults` optionally overrides subsequent changes in one sequence.
These are shared library defaults, not mutable global process settings. The
low-level MotionGfx API keeps its existing explicit timing behaviour.
`Sequence::wait` inserts an existing silent fragment. `Sequence::play_all`
starts an iterator or array of changes together and waits for the longest.
Its elements must have the same change type, such as many rectangle x moves.
`Sequence::parallel(|sequence| { ... })` groups any mixture of `play`,
`play_all`, or nested blocks at the same start time, including different
subject types. Sequence defaults set inside a block remain in effect after it.
`Sequence::chain` builds one sequential branch inside a parallel block.
`Sequence::stagger(interval, changes)` starts successive changes at fixed
intervals and waits for the latest end, using the existing `ord_flow` operation.
`Sequence::staggered(interval, |sequence| { ... })` is the scoped form:
each authored action or nested group is one child, with fixed spacing between
child starts. Use nested `parallel` blocks to start batches together and
`chain` blocks to stagger whole sequential branches. It returns the group's
reserved duration, excluding preceding actions; an empty block returns zero.
Like `parallel` and `chain`, it collects fragments during construction and
delegates scheduling to core ordering, adding no playback evaluator.
For custom ordering, lower descriptions with `Sequence::action`, order those fragments
with MotionGfx's `ord_all` / `ord_flow`, then `Sequence::append` the result.
`TrackFragment::duration()` reads a fragment's reserved span before it is
appended, useful for a playback chapter boundary without repeating timing
constants. Resolving conflicting actions during compilation can extend that
span, so it is not a replacement for the final compiled track's duration.
The first draft deliberately reuses those combinators. No procedural macro
or new timeline implementation is involved.

`Property::discrete` uses step interpolation for values such as text content.
`sequence.play(property.set(value))` adds a zero-duration action at the current
sequence time, so replacements also reconstruct correctly when seeking.
`sequence.set(property, value)` is equivalent. Constructing a change alone does
not schedule it; changes are marked `must_use` to warn about discarded actions.
`Property::to` and `Sequence::set` accept values convertible into the target
type, allowing string literals for shared-string text properties.

This crate has no Vello, Winit, or scene-document dependency. Custom Rust
subject types can expose their own typed `Property` fields.

## Shared scene lifecycle

`Scene<M>` owns an application-selected model `M`, one native binding plan,
and the ordered native update callbacks. `World<M>` wraps the model with
scene-local signal storage and sampled time. Models implement the core
`SubjectSource<ModelId, ModelState> for World<M>` using model-owned typed IDs.
There is no rendering dependency, closed list of shape kinds, or per-sample
type registry in this shared layer.

`SceneObject<M>::insert_into` registers state and returns typed handles.
`ObjectDefinition<T, M>` supplies reusable descriptor/property wiring; model
crates may wrap it to expose their own inherent builders and input conversions.
`Input<M, T>` prepares fixed/signal/computed/property inputs. `SampleTime`
provides the clamped sample time as an explicit source through `scene.time()`.

`scene.try_animate(|sequence| ...)` creates the existing core registry/timeline,
validates the binding plan, bakes relative targets, and samples time zero.
`animate` is the panicking convenience. `Animation<M>::sample(time)` applies
the core timeline, evaluates bindings, then runs direct callbacks in registration
order. `model()` exposes a read-only sampled model; `state()` also exposes
shared time and registered signal inspection. Renderers consume these views
separately and own their own caches/resources.

`UpdateTargets<M>` prepares a native callback with model-specific safe target
borrowing. The optional `UpdateStorage`/`UpdateHandle` contracts implement
array/tuple target collections without a renderer-specific collection engine.
Direct callbacks run on every sample, have no dependency/writer sorting, and
must assign from sampled inputs for arbitrary-time seeking. Mutable models
and callbacks remain native Rust; this does not add a scripting runtime.

`prepare_with<Key>(...)` is an integration hook for final configuration such
as last-setter-wins backgrounds. It runs during construction and is keyed by
type only there. It is not a per-frame system registry.

See [headless shared model tests](tests/shared_scene.rs) and the
[portable 2D model](../motiongfx_2d/README.md) for complete adapters. Standalone
`Sequence<W>` and `BindingPlan<W>` remain usable without this scene lifecycle.

## Native derived relationships

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
uses compiled action metadata, including raw appended fragments. Whole fields
overlap their nested axes; sibling axes are independent. Bound properties can
feed other bindings, even when consumers were registered before producers.

Call `evaluate(world)` after each sample. A fixed dependency order is reused;
there is no per-frame graph sorting. Shared derived outputs reuse their storage
after initialization. Native closures still have execution/dispatch costs, and
input reads, comparisons and destination writes are additional work. Evaluation
does not use elapsed frame time; deterministic functions reconstruct their
results during backward seeking. Interior mutable state, wall-clock reads,
randomness and numerical validity are not statically checked.

This is optional native binding support, not automatic dependency tracking or
a scripting interpreter. Ordinary property handles still do not identify their
owning scene; mixing handles between scenes is unsupported. The shared `Scene<M>`
owns the plan and finalizes it through `try_animate` (returns `BindingError`)
or `animate` (panics on a binding error during construction).

## Standalone sources

`signal(initial)` / `Signal::new(initial)` and
`computed(inputs, function)` / `Computed::new(inputs, function)` construct
scene-independent descriptions. `signal.map(function)` and
`computed.map(function)` describe single-source calculations. `Property::map`
also supports sampled native properties. These retain owned dependencies; a
helper can return a description without retaining references to its arguments.

`Source<W>::register(world, plan)` prepares a description for one world and
deduplicates its computed nodes. Integrations implement `SignalWorld` with
`SignalValues` storage and `SubjectSource<SignalId, T>` access, and construct
`Sequence::with_signals` so even roots used only in actions are registered.
The shared scene lifecycle does this automatically; `computed!` is provided here to
avoid repeating dependency names. Regular core-property sequences still use
`Sequence::new` without signal storage.

Descriptions are cloneable shared ownership, not `Copy` runtime IDs. Each
world has independent sampled values and caches. Registration maps and source
descriptions are not traversed during sampling; evaluation retains the native
source reads, comparisons and closure dispatch described above. This is explicit
dependency wiring, not automatic tracking, effects, or an interpreter.

## Optional authoring macro

`animate!` is a `macro_rules!` convenience in this crate, also re-exported by
`motiongfx_2d` and `vello_motiongfx`. It introduces no new crate, parser dependency, or per-frame
execution layer. The first argument is a mutable `Sequence` reference:

```rust
animate!(sequence, {
    status.content.set("Starting");
    square.position.x.to(100.0).over(ms(300));
    wait(ms(200));

    parallel {
        square.rotation.offset(0.4).over(ms(500));
        chain {
            square.fill.to(ORANGE).over(ms(200));
            square.scale.multiply(0.5).over(ms(300));
        }
    }

    stagger(ms(100), bars.iter().map(|bar| {
        bar.position.y.offset(40.0).over(ms(300))
    }));
});
```

Animation expressions become `sequence.play(...)`; `wait`, `parallel`,
`chain`, and `stagger` use the same plain Rust builder methods. `parallel`
also accepts an iterator/array: `parallel(changes);` expands to `play_all`.
Collection forms require same-type changes, just like their builder methods;
blocks accept mixed types. Both group blocks allow an optional trailing `;`.
Statements are sequential outside groups. A zero-duration `set` does not
advance time. Stagger spacing is between starts, so three 300ms animations
with a 100ms interval finish after 500ms.

This initial macro handles animation expressions and timing forms, not general
Rust statements. Keep `let`, `for`, `if`, defaults and ordinary helper calls
outside it. A Rust loop or helper can contain `animate!`; a helper may receive
the same mutable sequence and
append any number of actions. Plain Rust and macro calls can be mixed. Normal
Rust type checking applies to the expanded calls. A function's returned
animation description is valid as an animation expression inside the macro.
