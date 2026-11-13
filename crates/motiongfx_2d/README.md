# MotionGfx 2D

Objects, styles, geometry, attachments, and 2D camera semantics live here. This
crate depends on Kurbo/Peniko directly, not Vello, Winit, or a GPU. The
`Scene` and `Animation` names are convenience aliases for the shared
`motiongfx_authoring::Scene<Graphics>` / `Animation<Graphics>` lifecycle.
Import `motiongfx_2d::prelude::*` for objects and the 2D extension traits
(`Scene2dExt` for camera/background setup, `Animation2dExt` for sampled reads).
The portable model makes no claim that every renderer already supports it.

`Object<T, M>` and `Input<T, M>` are thin local adapters over shared authoring
preparation. They supply shape setters and tuple/geometry conversions without
reimplementing the timeline or dependency evaluator. The default model is
`Graphics`. Constructors normally infer it from `scene.add(...)`; a detached
builder used without a scene may need `let shape: Object<Circle> = ...`.
Components can construct builders without borrowing a scene.

For composite models use `motiongfx_authoring::Scene<MyModels>` and implement
`Model2d` to expose a contained `Graphics` store. The same shapes, signals,
computed descriptions and typed core properties work in that shared scene.
See [the headless composite model test](tests/composite_model.rs).
Collection callbacks use the shared `UpdateStorage`/`UpdateHandle` contracts;
the provided 2D collection adapters currently target `Graphics`. A composite
model supplies its own safe borrowing adapter or specific `UpdateTargets`
implementation. It does not need another timeline.


See [architecture and direction](../../docs/architecture.md) for the shared
authoring design, performance boundaries and future reactive/editor options.

Portable 2D model and typed authoring with rectangles, circles, lines, stroked paths and text,
used by `moving_square`, `insertion_sort`, `commit_graph` and `sine_unit_circle`. `Scene::add` creates stored object
state and returns typed authoring properties. Shape constructors produce
`Object<T>` authoring descriptors; `scene.add` stores their plain state and
registers any driven fields. Handles belong to the scene
that created them; do not mix handles between scenes.

`scene.set_background(color)` sets the renderer's clear/base color, without creating
geometry. It accepts a fixed `Color`, signal, or computed color; the last call
wins. The default remains RGB(28, 28, 46). For a fixed initial color, retain the
returned property to animate it with ordinary sequences. When using a signal,
animate that signal. The shared Winit host reads `Animation::background()` after
sampling and passes it to `RenderParams::base_color`. Other rendering hosts
should do the same. The background fills the render target regardless of
camera transforms; playback controls retain their own painted background.

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

### Shared text styles

```rust
let size = signal(56.0);
let labels = TextStyle::new()
    .font_size(&size)
    .align(TextAlign::Center);

scene.add(Text::new("Radius").style(&labels).fill(INK));
scene.add(Text::new("Area").style(&labels).fill(RED));
// Within the timeline:
sequence.play(size.to(72.0));
```

`TextStyle` is a typed partial preset with optional `font_size`, `align`, `fill`,
and `opacity` inputs. Its constructor applies no defaults. `.style(&preset)`
applies only configured fields; subsequent styles or direct setters override
earlier values and replace obsolete reactive wiring. Content, position,
transforms and parent attachment stay on the individual object.

Styles accept fixed values, signals, computed values and matching property
sources, using the same setters/binding registration as explicit object code.
A fixed style establishes initial values that can be animated per object. For
fields driven by a shared signal, animate that signal instead. The preset itself
is not an animation target or a live theme: changing it after application does
not change already-created objects. Clone a preset to derive a variant.

`ApplyStyle<T>` restricts each preset to compatible object types at compile time.
Currently `TextStyle` implements `ApplyStyle<Text>`; other style families can
opt in when needed. Style and object setters share one property declaration,
and applying a preset goes through existing setters rather than a second
property system. Inputs are retained during construction; no style object or
style-resolution pass is stored in playback. Shared reactive inputs have their
usual binding evaluation costs. No CSS cascade, selectors, layout, automatic
inheritance or new crate is introduced.

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

Text uses real font shaping through [motiongfx_text](../motiongfx_text/README.md)
and Vello glyph runs. Fira Sans is embedded by default; `.font_family("...")`
selects a registered family. `TextStyle` shares font/layout/paint inputs just
like explicit setters. `font_weight` accepts numeric values or `FontWeight`
constants; `font_style` selects normal/italic/oblique. Line height is a font-size
multiplier, letter spacing is in canvas units, and wrap width zero is unconstrained.
Position anchors the block's top; `TextAlign` anchors its total width horizontally.
It does not set per-paragraph alignment or provide a general layout system.

`text.select(&mut scene, "word")?`, `select_nth` and `select_range` register typed
fill/opacity targets. Literal selection uses the initial fixed source; animated
content has no automatic selection remapping. Invalid/missing selections return
`SelectionError`. Later overlapping selections win. A selection initially copies
the text's fill, then owns its fill independently; opacity multiplies text opacity.
Changed content disables the selection until the same source returns on a seek.
Partial ligatures paint as whole clusters. Selections are metadata, not additional
scene objects. Whole-object position/scale/rotation continue to work normally.

Renderer-owned layout is cached by content, font and layout inputs; paint and
transform animation do not reshape. Font size or weight changes can reshape,
while scale reuses glyph layout. Source strings are `Arc<str>`, avoiding copies
of their allocations when sampling. Advanced write-in, per-part transforms and
text replacement/matching transitions are future work, not implemented features.

`Scene::animate` consumes the scene, builds a MotionGfx timeline and
bakes its targets. `Animation::sample` uses the existing MotionGfx sampler;
The separate `vello_motiongfx::Renderer` emits Vello scene commands from sampled state. Object state is stored
once, with geometry transforms calculated when rendering. There is no
per-frame authoring interpreter. This does not remove ordinary MotionGfx
subject access, interpolation, or Vello command-recording costs.

Use centered logical canvas coordinates, y upward, and counterclockwise
radians. Rectangle scale changes geometry while keeping stroke width fixed.
Fill and stroke colors are independent. `smooth` reproduces Manim's default
normalized sigmoid easing for comparisons.

`Line::new(start, end)` stores endpoints without allocating a path buffer.
Its handle exposes `start`/`end` (including `.x`/`.y`), stroke, stroke width,
opacity and `reveal`. Endpoints use positive-y-up coordinates and receive the camera
transform like all other objects.

Lines support `.line_dash([20.0, 20.0])`, `.dash_offset(value)`, and fixed-distance
`.start_offset(value)` / `.end_offset(value)` trimming. Offsets trim the full line
first, then `reveal` selects its visible prefix. The shaft uses native
Vello/Kurbo dashing. Numeric fields are tweenable; entire dash patterns use
discrete replacements.

Stroke builders also expose caps and joins, using Vello/Kurbo's typed enums:

```rust
use motiongfx_2d::{Cap, Join};

Line::new((0.0, 0.0), (200.0, 0.0))
    .line_dash([20.0, 20.0])
    .line_cap(Cap::Butt) // Flat stroke/dash ends; also Square or Round.
    .line_join(Join::Miter) // Sharp stroke corners; also Bevel or Round.
    .stroke(INK, 8.0);
```

Both default to `Round`, preserving existing appearance. They are available on
lines, paths, triangles, chevrons and rectangles, including their inline tips.
Caps apply to open ends and dash ends; joins apply between stroke segments.
They have no effect on fills, and caps have no effect on closed outlines.
Full circles expose neither option: their outline is closed and smooth. Future
open arcs or dashed circles can expose meaningful caps when implemented; no
unused cap/join fields are reserved on `Circle`. Shared stroke-paint code stays
separate from optional cap/join builders. Miter joins retain Kurbo's default miter
limit of 4. A line's stroke settings do not override its tips' own settings.

Like other properties, these accept signals/computed inputs. Handle changes are
discrete, not smoothly interpolated:

```rust
sequence.play(line.line_cap.set(Cap::Square));
sequence.play(line.end_tip.line_join.set(Join::Round));
```

Round caps/joins derive their rounding from stroke width. They are not a numeric
corner-radius control. A triangle's fill remains sharp with a round stroke join
alone. Use its separate geometry `.radius(...)` to round the actual boundary,
as with a rectangle's `.radius(...)`.
For an open chevron, round caps and joins can round its existing stroked shape
without adding filled geometry.

### Inline endpoint shapes

```rust
let line = scene.add(
    Line::new((0.0, 0.0), (200.0, 0.0))
        .start_tip(Circle::new(6.0).fill(RED))
        .end_tip(
            Triangle::new()
                .size(24.0)
                .angle(0.6)
                .fill(RED)
                .stroke(INK, 2.0),
        )
        .stroke(INK, 4.0),
);
sequence.play(line.start_tip.radius.to(10.0));
sequence.play(line.end_tip.angle.to(0.9));
sequence.play(line.end_tip.fill.to(ORANGE));
```

`start_tip` / `end_tip` accept ordinary `Object<Circle>`, `Object<Triangle>` or
`Object<Chevron>` descriptions. No separate `scene.add` is needed. The old
boolean arrow flags and shared `arrow_size` have been replaced by this API.
The returned handle retains the concrete tip types: a circle exposes `radius`,
while triangles and chevrons expose `size` and `angle`. Triangle also exposes
its independently animatable corner `radius`. Ordinary line fields
remain accessible through the same handle. `Animation::line` accepts either
plain or tipped line handles; `circle`, `triangle`, and `chevron` access sampled
tip state. Tipped lines and their shapes also work with direct updates.

`Triangle::new()` and `Chevron::new()` default to a 24-unit axial length and a
pi/3 full opening angle. Their local apex is at the origin, pointing along +x;
the base corners are `(-size, ±size * tan(angle / 2))`. Triangle closes/fills the
outline; chevron is an open stroked outline (its fill setting is unused).
Sizes must be finite/nonnegative and angles strictly between zero and pi.
Setters accept fixed values or reactive inputs, just like other shape builders.
Both shapes are independently usable with `scene.add`, with the same geometry,
styling and render implementation as their attached versions.

Triangle supports true rounded geometry, with the same boundary for fill and
stroke:

```rust
Triangle::new().size(24.0).angle(0.6).radius(2.0).fill(INK)
// On a registered triangle tip:
sequence.play(line.end_tip.radius.to(4.0));
```

Radius defaults to zero (the original sharp triangle), accepts reactive inputs,
and is measured in local canvas units. Negative radii behave as zero; nonfinite
values are rejected. Rendering clamps the effective radius to the inradius
(the largest circle that fits inside the triangle), preventing corner overlap.
The requested radius property is retained, so size/angle changes can change the
effective limit. Corners are circular fillets represented by cubic path curves.
The rounded path stays inside the original triangle. Position still anchors the
nominal sharp apex, so rounding cuts back its visible point rather than moving
the object. Tip shaft trimming uses the actual curved boundary, including local
rotation/scale.

The Vello renderer caches local triangle geometry outside sampled state and
rebuilds it only when size, angle or radius changes. Styles and transforms reuse it. Geometry uses
fixed-capacity storage without a per-frame heap path allocation. Chevron stays
an open stroked shape with cap/join controls and does not expose a radius field.

The portable `Line::layout` places each tip in an outward-facing endpoint frame after offsets and
reveal. Shape position/rotation are local offsets within that frame; scale also
scales its stroke. Circle position anchors its centre; pointed shapes anchor
their apex. The line reserves the inward geometric extent for circles/triangles;
an open chevron lets the shaft meet its apex. Stroke thickness does not expand
that trim calculation. Both tips shrink uniformly when necessary to fit a short
visible segment, without changing sampled shape state or reversing the shaft.
Degenerate/fully trimmed lines and zero reveal hide the tips too.

Tip styles are independent of shaft style. Line opacity multiplies tip opacity;
transparent or zero-width shafts can still show tips. Attachments use shared
shape pools internally, have no independent draw-order entry, and render with
their line (shaft, start tip, end tip). A tip cannot also use `relative_to`:
the line supplies its parent frame. Copying a descriptor creates independent
instances; use signals for shared animated values. Built-in tip geometry uses
stack data with no extra path-buffer allocation. No per-tip update callback is
needed to follow the line. Arbitrary custom-object attachments are not yet
supported.

## Signals and computed properties

```rust
use motiongfx_2d::{computed, signal};

let progress = signal(0.0);
let radius = signal(80.0);
let position = computed!((progress, radius) => {
    let angle = progress * std::f64::consts::TAU;
    return (radius * angle.cos(), radius * angle.sin());
});
let dot = scene.add(Circle::new(6.4).position(&position).fill(YELLOW));
let line = scene.add(Line::new((0.0, 0.0), &position).stroke(BLUE, 2.0));

// Single-source calculations can be inline, without borrowing scene.
scene.add(Circle::new(radius.map(|r| { return r * 2.0; })));
```

`Signal<T>` is writable timeline state; `Computed<T>` is read-only, cached
native math. Both are standalone, cloneable descriptions, not `Copy` IDs or
mutable cells. Pass references to reuse them; retained dependencies are owned
internally. Each scene registers its own independent sampled state. Helpers can
return reactive descriptors without taking `&mut Scene`. Object properties are
sources too, but their handles remain scene-local.

`signal(initial)` is shorthand for `Signal::new(initial)`. Likewise,
`computed(inputs, calculate)` calls `Computed::new(inputs, calculate)`.
`computed!((progress, radius) => { ... })` expands to
`computed((&progress, &radius), move |(progress, radius)| { ... })`.
It removes repeated names while keeping dependencies explicit and fully typed.
For a single expression, omit the block:
`computed!((progress, radius) => progress * radius)`.
Both explicit `return` and Rust's final-expression return work. Position builders
accept either a `Point` or a computed coordinate tuple.

The old scene-owned constructors have been replaced, not retained as aliases.
Schedule signal changes with the same `sequence.play`,
`.to`, `.offset`, `.multiply`, `.over`, and `.ease` APIs as object properties.
`signal.set(value)` describes a zero-duration action, not an immediate write.

Storage requires `Clone + Send + Sync + 'static`; comparison-based sources
also require `PartialEq`. Numbers, points and colors have existing tween
interpolation. Arrays, vectors, booleans, strings and custom types can be
stored and replaced with `.set`, or used in computed values. Tweening those
requires a meaningful interpolation implementation; generic array/vector
interpolation is not added here. Large source values may incur cloning and
comparison work; use shared buffers or direct mutation where appropriate.

Circle, rectangle, line, text, path and camera descriptor setters accept fixed
values, signals, computed values and matching object-property sources. Position
also accepts fixed tuples and computed `(f64, f64)` outputs. Constants create
no binding. Repeated setters replace earlier wiring for that field. Signal
relationships are stripped from the descriptor during insertion: stored object
fields remain plain points/numbers/colors, with evaluation in the existing
binding plan. Clone a descriptor to create objects that share the same inputs.

Standalone descriptions register when used by object builders, bindings,
updates, or timeline actions. Unused descriptions never evaluate. Registration
deduplicates shared calculations per scene; playback uses prepared indices,
without description lookups or dependency sorting. No dependency reads are
tracked automatically. Registered calculations evaluate once per changed input
tuple. The plan still reads/compares inputs on each sample and dispatches native
closures; it is not zero-cost or a lazy UI signals framework. Construction adds
shared description allocations and registration maps.

`Path::quadratic(start, control, end)` accepts sources for any control point.
Its exact geometry is cached until those points change; reveal/style animation
does not regenerate the source path. `Line::reveal` exposes a normalized visible
fraction without creating a path buffer, useful for animated grid lines.

## Advanced explicit bindings

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
Properties, `Computed<T>` handles, and tuples of two to five sources are
supported. All values are local object state, not transformed world positions.
For a shared calculation use `let orbit = curve.end.map(function)`;
then `scene.bind(dot.position, &orbit, |position| position)`. Its output is cached
once per changed input for all consumers. Tuples support one to five sources.

`scene.try_animate(...)` returns `Result<Animation, SceneError>` and rejects
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
let progress = signal(0.0);
let dot = scene.add(Circle::new(6.4).fill(YELLOW));
scene.update(dot, &progress, |circle, progress| {
    let angle = progress * std::f64::consts::TAU;
    circle.position = Point::new(80.0 * angle.cos(), 80.0 * angle.sin());
});
```

`signal(initial)` returns a typed, standalone animation property. It is
not a drawable or a dependency node. Animate it with the existing `.to`,
`.offset`, `.over`, and sequence methods. Sampled values use scene-local storage;
the descriptions themselves can be reused across scenes independently.
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
The Bézier example now uses computed inputs and source-aware builders, with
no direct updates. `Path::quadratic` and `set_quadratic` use exact
quadratic segments, retaining the normal `start`/`end` reveal properties.

## Springs

Continuous scalar properties and Point/Vec2/Size support `.spring_to(value, settings)`
through core spring actions. Import `Spring` from this
crate or its prelude. There are no spring-specific renderer changes.
See [spring options, normalized rest units and limits](../../docs/springs.md).
