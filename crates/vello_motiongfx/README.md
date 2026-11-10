# Vello MotionGfx

Initial native graphics integration, currently implementing rectangles and text for
the `moving_square` and `insertion_sort` comparisons. `Scene::add` creates stored object
state and returns typed authoring properties. Handles belong to the scene
that created them; do not mix handles between scenes.

`Rectangle::new(width, height)` applies the other defaults internally. Optional
`.fill(color)` and `.stroke(color, width)` calls customize the initial state;
struct literals with `..Rectangle::default()` remain supported. Shared
`colors::{BLUE, ORANGE, WHITE, ...}` constants avoid repeated RGB definitions.
`Rectangle::from_size(size)` also accepts a tuple such as `(160.0, 160.0)`,
a `Vec2`, or a `Size`.
`.position((x, y))` sets the initial center and `.radius(value)` sets a corner
radius. Both text and rectangle handles expose `position.to(...)` and
`position.offset(...)`, plus `position.x` and `position.y` animation targets.
Avoid overlapping whole-position and per-axis actions on
the same object at the same time; they write to overlapping fields.

`Scene` owns the objects and gives them identities for MotionGfx targeting;
it is not a renderer or window. A standalone rectangle is just initial state.
Registering it with `Scene::add` returns its typed property handles.
`Scene::animate` consumes that collection so baking sees the same objects
and initial state that playback will own; the callback runs once during setup.
There is one finalizing `animate` call per scene in this draft. Compose as many
actions or reusable Rust authoring functions as needed inside that call; they
all contribute to the same timeline. Independently compiled `Animation`
instances do not yet have a merge/concatenation API.

`Scene::add` uses the `SceneObject` trait to select registration from the
argument's type at compile time. Additional built-in shapes can implement this
same contract; reusable components can implement it by adding constituent
objects and returning their own component handle. No shape-specific public
insertion method or per-frame registration dispatch is required.

Plain text is an ordinary scene object:

```rust
let title = scene.add(
    Text::new("Insertion sort")
        .position((-400.0, 250.0))
        .font_size(24.0)
        .fill(WHITE),
);
```

`Text` exposes `position`, `font_size`, `fill`, `opacity`, `scale`, `rotation`
and `content` targets. `sequence.play(title.content.set("Sorted"))` replaces
content at the current sequence time without adding duration. The equivalent
`sequence.set(title.content, "Sorted")` remains available. Alternatively,
`sequence.play(title.content.to("Sorted").over(s(1)))` holds the old content
until the change's end. These are discrete replacements, not glyph morphs.
`Animation::text(handle)` reads sampled text state.

Text position anchors the top of the block. `TextAlign::Left` (default),
`Center`, or `Right` controls horizontal anchoring; positive scene y points up.
Mixed rectangles and text render in their `scene.add` order.

`Text::new("5").relative_to(bar).position((0.0, -40.0))` anchors text to a
rectangle's center. Its position is then a local offset; it inherits the
rectangle's translation, rotation and scale automatically, without extra
animation tracks or cache rebuilds. The text's own position/scale/rotation
remain independently animatable, and its color/opacity stay independent.
The initial anchoring implementation supports rectangle parents in the same
scene; this is not a general hierarchy or layout engine.

The temporary renderer is still 5x7 vector lettering, **not a real font**.
ASCII letters display in uppercase, newline starts a new line, and unsupported
characters use a question mark. `font_size` is the seven-cell glyph height in
logical canvas units. No font selection, shaping, Unicode layout, or typesetting
integration is implemented yet.

Text state/handles live in `text.rs`; cached rendering lives in
`text_renderer.rs` and `lettering.rs`. Geometry is separate from cloned/sampled
state, and rebuilds only when content changes. Font size currently scales the
unit geometry, so font-size, transform, color and opacity animations reuse it.
Content is stored as `Arc<str>` so sampling does not copy string allocations.
This separation allows replacing the lettering/layout code with proper font
rendering later while preserving the basic authoring API. Font-dependent
layout would need to include its font/style inputs in its cache key.

`Scene::animate` consumes the scene, builds a MotionGfx timeline and
bakes its targets. `Animation::sample` uses the existing MotionGfx sampler;
`Animation::render` emits normal Vello scene commands. Object state is stored
once, with geometry transforms calculated when rendering. There is no
per-frame authoring interpreter. This does not remove ordinary MotionGfx
subject access, interpolation, or Vello command-recording costs.

Use centered logical canvas coordinates, y upward, and counterclockwise
radians. Rectangle scale changes geometry while keeping stroke width fixed.
Fill and stroke colors are independent. `smooth` reproduces Manim's default
normalized sigmoid easing for comparisons.

The implementation is intentionally small: circle/path components and a
general procedural-update hook are not implemented yet.
