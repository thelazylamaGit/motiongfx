# MotionGfx Authoring

Native, typed setup conveniences over existing MotionGfx. `Property` stores
a subject ID, typed field path and interpolation function. `to`, `offset`,
`multiply` and `map_target` create descriptions; `Sequence::play` lowers them
to normal actions. Relative targets use the action-start value during baking.
No additional property evaluator runs during playback.

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
intervals and waits for the last end, using the existing `ord_flow` operation.
For custom/staggered ordering, lower descriptions with `Sequence::action`, order those fragments
with MotionGfx's `ord_all` / `ord_flow`, then `Sequence::append` the result.
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

## Optional authoring macro

`animate!` is a `macro_rules!` convenience in this crate, also re-exported by
`vello_motiongfx`. It introduces no new crate, parser dependency, or per-frame
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
outside it. A Rust loop or helper can contain `animate!`, as shown by the
insertion-sort example; a helper may receive the same mutable sequence and
append any number of actions. Plain Rust and macro calls can be mixed. Normal
Rust type checking applies to the expanded calls. A function's returned
animation description is valid as an animation expression inside the macro.
