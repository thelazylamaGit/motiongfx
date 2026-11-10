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
