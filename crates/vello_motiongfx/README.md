# Vello MotionGfx

Initial native graphics integration, currently implementing rectangles for
the `moving_square` comparison. `Scene::add` creates stored object
state and returns typed authoring properties. Handles belong to the scene
that created them; do not mix handles between scenes.

`Rectangle::new(width, height)` applies the other defaults internally. Optional
`.fill(color)` and `.stroke(color, width)` calls customize the initial state;
struct literals with `..Rectangle::default()` remain supported. Shared
`colors::{BLUE, ORANGE, WHITE, ...}` constants avoid repeated RGB definitions.
`Rectangle::from_size(size)` also accepts a tuple such as `(160.0, 160.0)`,
a `Vec2`, or a `Size`.

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
