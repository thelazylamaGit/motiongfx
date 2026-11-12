# Vello MotionGfx

Native graphics integration with rectangles, circles, lines, stroked paths and text,
used by `moving_square`, `insertion_sort`, `commit_graph` and `sine_unit_circle`. `Scene::add` creates stored object
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
scene, and now circle parents too. Circles can also use `relative_to(parent)`
to build anchored rings and reusable components. Parents must already exist
in the same scene. Only transforms are inherited; color and opacity remain
independent. This is not a layout engine.

`Circle::new(radius)` creates a filled circle. Use `.fill(TRANSPARENT)` and
`.stroke(color, width)` for a ring. Its handle exposes nested position axes,
radius, fill/stroke, stroke width, opacity, scale and rotation. Circle scale
transforms the stroke as well as the geometry, including anchored children.
Text can anchor to a circle using the same `relative_to` method.

`Path::new(points)` creates a polyline; `Path::from_bez_path(geometry)` accepts
arbitrary quadratic/cubic geometry. `.stroke(color, width)` stores its style,
and `.end(0.0)` initially hides it. Animate `path.end.to(1.0)` to reveal it.
`start`/`end` are normalized visible-range fractions. They reuse Peniko's
existing segment-based tracing, not arc-length parameterization. Position,
scale, rotation, opacity and stroke properties are also animatable.

Path source geometry is shared through `Arc<BezPath>` so timeline baking does
not clone large point buffers. Visible geometry is cached separately, outside
sampled state, and rebuilt only when the source or visible range changes.
Moving, fading or changing the stroke style of a path reuses its geometry.

Every scene has one camera, initially centered with frame scale 1 and rotation 0.
`scene.camera()` returns its typed `position` (including `.x`/`.y`), `scale`
and `rotation` targets. Schedule them with ordinary `sequence.play` and
`sequence.parallel` calls. Frame scale 0.25 shows a quarter of the normal
width/height, giving four times the magnification. Positive
camera rotation is counterclockwise, so the world rotates clockwise.
Frame scale must remain finite and positive, and position/rotation must be finite.
The shared view transform leaves geometry caches unchanged; all scene objects
receive it, while the host's playback controls remain in screen space.

For an attached view, configure initial state before `animate`:

```rust
let camera = scene.set_camera(
    Camera::new().relative_to(circle).position((0.0, 80.0)),
);
```

The target must already exist in this scene. Attachment follows its sampled
world position, including ancestor transforms, but does not inherit its scale
or rotation. Camera position is an offset in world axes. Resolution happens
when rendering after timeline sampling; it has no accumulated follow state,
so seeking works in either direction. `Animation::camera()` reads sampled
camera state, whose position remains the local offset when attached.
See `camera_zoom` for a Manim-style zoom to a circle's tangent and back.
It interpolates frame scale and position together, then derives magnification
as `1 / scale` during rendering. This matches the reference's frame-size
interpolation. There is one scale value, rather than independent zoom and
frame-scale properties that could conflict.

The optional `animate!` macro is re-exported here for native authoring. It
removes repeated `sequence.play(...)` calls and supports sequential changes,
waits, parallel blocks/collections, chained branches, and staggered collections.
It expands to the same builder API and does not change playback or rendering.
See `crates/motiongfx_authoring/README.md` for its syntax and scope.

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

`Line::new(start, end)` stores endpoints without allocating a path buffer.
Its handle exposes `start`/`end` (including `.x`/`.y`), stroke, stroke width
and opacity. Endpoints use positive-y-up coordinates and receive the camera
transform like all other objects.

## Native property bindings

```rust
scene.bind(dot.position, curve.end, move |reveal| {
    let angle = reveal * turns * std::f64::consts::TAU;
    Point::new(-320.0 + 80.0 * angle.cos(), 80.0 * angle.sin())
});
scene.bind(radius.end, dot.position, |position| position);
scene.bind(connector.end, (curve.end, dot.position.y), move |(reveal, y)| {
    Point::new(-240.0 + reveal * turns * 320.0, y)
});
```

Closures that capture local constants/configuration use `move` to own those
captures. They are stored, not executed during the author callback. A source
supplies its current sampled value, not an implicit progress parameter.
Properties, `Derived<T>` handles, and tuples of two to five sources are
supported. All values are local object state, not transformed world positions.
For a shared calculation use `let orbit = scene.derive(curve.end, function)`;
then `scene.bind(dot.position, orbit, |position| position)`. Derived handles are
copyable, and their output is cached once per changed input for all consumers.

`scene.try_animate(...)` returns `Result<Animation, BindingError>` and rejects
cycles, overlapping binding destinations, and timeline writes to bound fields.
`scene.animate(...)` keeps its existing return type and panics on these errors
during construction. Both initialize the sampled animation and bindings at
time zero. Bound fields can feed other bindings; execution order is prepared
once regardless of registration order. Ordinary property handles still must
belong to this scene; scene identity validation is not implemented.

`Animation::sample` runs MotionGfx first, then bindings, then direct updates;
rendering reads
the resulting state. Changed inputs execute native Rust closures; unchanged
inputs skip calculation/writes. This adds source reads/comparisons and native
closure dispatch; it is not zero-cost and not a scripting interpreter. Binding
functions must be deterministic functions of their declared inputs. Stateful
dt updaters, automatic dependency tracking and runtime rebinding are not
implemented. The sine example needs none of those: it reveals precomputed
geometry and derives the dot/line endpoints from the same sampled reveal.

## Direct object updates

```rust
let progress = scene.value(0.0);
let dot = scene.add(Circle::new(6.4).fill(YELLOW));
scene.update(dot, progress, |circle, progress| {
    let angle = progress * std::f64::consts::TAU;
    circle.position = Point::new(80.0 * angle.cos(), 80.0 * angle.sin());
});
```

`scene.value(initial)` returns a typed, standalone animation property. It is
not a drawable or a dependency node. Animate it with the existing `.to`,
`.offset`, `.over`, and sequence methods. Values use scene-local storage, so
their handles have the same ownership restriction as object handles.
`scene.time()` is an explicit source of sampled `Duration`, clamped to the
timeline range, not elapsed frame time.

`scene.update(targets, sources, callback)` registers an ordinary `FnMut` Rust
function. It receives mutable objects directly and source values by value.
Targets may be one object handle, a tuple of two to four handles, or an array
of one handle type. Group/array order matches the caller's order, even when it
differs from storage order. Duplicate mutable targets and missing sources are
rejected with a construction-time panic. Target borrowing uses safe disjoint
slice access; no `unsafe`, runtime borrow cells, or per-sample target allocation.
Sources may also be tuples, existing bound properties, or shared derived values.

Callbacks run on **every sample**, including initialization, repeated times,
holds and backward seeks. They run after all bindings, in registration order.
There is no automatic dependency sorting, writer-conflict checking or input
caching for direct updates. Use the sampled inputs to assign positions rather
than incrementing previous positions if the animation must seek correctly.
A later callback may read an earlier callback's updated property; a binding
cannot depend on a later callback and see that write in the same sample. Avoid
giving both a binding and a callback ownership of the same output. Side effects
inside callbacks are not made deterministic by Rust's type system.

The callback incurs a stored function call plus source reads/object selection
and the math it contains; this is a thin native API, not a claim of zero cost.
For expensive work, cache explicitly or use the separate derived-value API.
The Bézier example caches readout formatting and edits quadratic geometry only
when control points change. `Path::quadratic` and `set_quadratic` use exact
quadratic segments, retaining the normal `start`/`end` reveal properties.
