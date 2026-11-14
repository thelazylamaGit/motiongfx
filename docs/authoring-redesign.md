# Authoring redesign: animations as values

Status: **proposal, not implemented.** [architecture.md](architecture.md) records
the current implementation. Nothing here is a commitment until reviewed.

## Goals

- Animation code that reads like Manim or Motion Canvas: describe objects,
  then describe what happens to them, with as little plumbing as possible.
- Composable: animations and components are ordinary values that functions can
  build, return, store and combine.
- Procedural: loops, iterators and conditionals are plain Rust.
- Plain Rust and good tooling: no per-arity trait machinery, no mini-language
  macros that break rustfmt or rust-analyzer.
- Keep core MotionGfx (timeline, baking, seeking) as the foundation.
- Leave a path to scripting, hot reload and node graphs through
  `motiongfx_scene`, without making Rust authors pay for it.

## Summary of decisions

1. **One animation value type, `Anim`.** Property changes, springs, waits and
   ordering groups are all `Anim`s, so arrays, `Vec`s and iterators of them work.
2. **Free combinators** `chain`, `all`, `any`, `flow(interval, ..)`, `wait(..)`
   take any `impl IntoIterator<Item = Anim>` (arrays, `Vec`s, iterators).
3. **No user-facing `Sequence`.** `scene.compile(steps)` takes a list of steps
   that run in order (an implicit top-level `chain`). `play`, `play_all`, the
   `parallel`/`chain`/`flow` callbacks, `defaults` and the `animate!` macro are
   removed. `compile` consumes the scene, as `animate` does today.
4. **Defaults by inheritance:** `.over(..)` / `.ease(..)` on a group set the
   default for children that don't set their own. Scene-wide defaults live on
   the scene.
5. **Named markers** (`marker("loop")`) replace returning durations to configure
   looping. The host still owns playback policy.
6. **Objects:** typed handles stay, for reasons given below. Composition gets
   child nesting (`.child(..)`) and value components through the existing
   `SceneObject` trait.
7. **Leaves are data** for built-in operations (to / offset / multiply / set /
   spring), with a Rust-only `Custom` closure escape hatch. Nothing
   scripting-specific (serialization, name registries, document conversion,
   computed-value graphs) is built now; the data leaves only avoid a later
   rewrite.
8. **Object composition:** `scene.add` accepts single objects,
   collections and iterators, `.child` / `.children` describe trees, plain
   functions and loops are components (props structs optional), an optional
   `Group` gives local coordinates, and `let` bindings act as refs.
   No object macro and no implicit global scene.
9. **No built-in entrance/exit helpers yet.** Visibility uses ordinary
   properties with explicit start states; reusable effects are functions
   returning `Anim`.
10. **Scene sections, not multiple scenes.** Transitions are animations of
    section parents; continuous effects use time-driven computed values or a
    timeline branch.
11. **A node tree in `motiongfx_2d`** (phase 2): every object is a node
    (transform, opacity, parent, ordered children, optional layout) plus a
    shape payload. Layout is a `layout` feature of `motiongfx_2d` using Taffy.

## 1. `Anim`: animations are values

```rust
pub enum Anim {
    Leaf(Leaf),                                  // one property change
    Ordered { order: Order, children: Vec<Anim>, defaults: Timing },
    Wait(Duration),
    Marker(Name),                                // zero-length named time
}
pub enum Order { Chain, All, Any, Flow(Duration) }
```

The enum is internal; users only see constructors and modifiers. A `Leaf` stores
the target (subject + field), an operation (`To`, `Offset`, `Multiply`, `Set`,
`Spring`, or `Custom`), the value, and optional duration and ease.

**Why one type?** Each current change is its own Rust type
(`Change<I, S, T, F>`, where `F` is a unique closure type). An array needs one
element type, so a mixed list of changes needs either tuples with one impl per
length (complex, worse errors) or type erasure: storing each change behind a
pointer so they all share one type. Erasure is the standard Rust answer for
heterogeneous lists (`Vec<Box<dyn Trait>>`). Core already does it for actions
(`Box<dyn Action<T>>`).

**Typed until erased.** `square.shift(..)` is still checked by Rust: the field
exists and the value type matches. Erasure happens inside the constructor, so
users never write `Box` or `.into()`.

**`Anim: Clone`.** Custom leaves store `Arc<dyn Fn>` rather than `FnOnce`, and
data leaves clone directly. That allows reuse (`let pop = ..; chain([pop.clone(), ..])`)
and repetition with `.repeat(n)`.

### Leaf storage: data with a `Custom` escape hatch

Core actions are functions `Fn(&T) -> T` (value at clip start to target value).
The built-in changes are common cases of that, so a leaf stores *what* the change
is rather than an opaque closure:

```rust
struct Leaf {
    target: PropertyKey,        // subject id + field
    op: Op,                     // To | Offset | Multiply | Set | Spring(params) | Custom(Arc<dyn Fn(&T) -> T>)
    value: Value,               // target value, type-erased (small values may be inline)
    duration: Option<Duration>,
    ease: Option<EaseFn>,
    lower: fn(&Leaf, &mut Lowering, Timing) -> TrackFragment, // fn pointer, one per value type
}
```

- **Performance:** same as closure leaves. Playback is identical, and setup does
  about one allocation per leaf either way. `lower` is a plain function pointer,
  not a heap closure.
- **Benefits now:** the tree is inspectable. `{anim:#?}` prints it, tests can
  assert on targets, and `anim.duration()` is straightforward.
- **Benefits later:** scripts and documents can only produce data, so built-in
  leaves never need rewriting for scripting or export.
- **`Custom`** is for changes that aren't a built-in op (for example rotating a
  point about the origin). It's Rust-only: it can't be saved or edited in a node
  graph, only shown as an opaque block. Like every core action, it runs once per
  clip during baking, never per frame.

Not built now: serde for values, name registries, document conversion. Those
belong to the scripting phase.

### Combinators

```rust
chain([a, b, c])            // one after another      (core chain)
all([a, b])                 // together, wait longest  (core all)
any([a, b])                 // together, reserve shortest (core any)
flow(ms(50), [a, b, c])     // starts 50ms apart       (core flow)
wait(ms(200))               // pause
marker("loop")              // named point in time, zero length
a.repeat(3)                 // a, a, a (finite)
```

They are free functions, not methods on arrays: they accept iterators directly,
read like Motion Canvas's `all(...)` / `chain(...)` and core's
`motiongfx::track::chain`, and avoid clashing with `Iterator::chain/all/any`.

**Bare arrays are not implicitly sequential.** `[a, b]` could plausibly mean
"together" (Manim's `self.play(a, b)`) or "in order" (Motion Canvas statements),
so the order is always named explicitly.

### Modifiers and defaults

- On a leaf: `.over(d)`, `.ease(e)`, `.spring(s)`.
- On a group: `.over(d)` and `.ease(e)` set the inherited default for children
  that don't specify their own. `.spring(..)` on a group is an error.
- Resolution order: leaf value, then the nearest group default, then scene
  defaults (`scene.defaults(ms(400), ease::cubic::ease_in_out)`), then library
  defaults.

### Errors

Most mistakes are still compile errors: wrong field, wrong value type, a
non-`Anim` in an array ("expected `Anim`, found `Property<..>`").

The one check that moves to runtime is springing a value that can't overshoot
(colors, strings, integers). After erasure Rust cannot ask "does `T` implement
`SpringValue`?". Each value type declares spring support once (the existing
`SpringValue` impls), `.spring()` is `#[track_caller]` so it records the
caller's file and line, and `compile` returns a `PreparationError` like:
`Color cannot spring (moving_square.rs:27); tween it with .over(spring.duration())`.

## 2. Entry point: `scene.compile(steps)`

`Scene` is the construction state (objects, signals, bindings). `compile`
prepares and lowers the steps, compiles and bakes the core timeline, prepares
bindings, and returns an `Animation`: a playback state that can be sampled at
any time, forwards or backwards. The current name, `animate`, suggests it plays
something; `compile` matches core terminology and says what happens. `Scene`
(what you author) and `Animation` (the compiled result) keep their names.

**Steps run in order.** `compile` takes `impl IntoIterator<Item = Anim>` and
chains it: the top level reads like a script. Nested groups must still be named
(`all`, `chain`, ..). A bare nested array is not an `Anim`, so it can't be
silently misread as either order. A single animation is written `[anim]`.

**One animation per scene.** `compile(self, ..)` consumes the scene: the world
moves into the `Animation` without cloning, and preparation hooks stay `FnOnce`.
Variants come from a function that builds the scene, called again.

**No `Sequence` callback.** Nothing becomes impossible; the trade-offs are small:
- procedural code collects steps (`steps.push(..)`, or `steps.extend([..])` for
  several) instead of calling `s.play(..)`, with the same number of lines;
- there's no mid-script "time so far" (`play` currently returns a `Duration`);
  markers cover the real use case, looping;
- changing defaults partway through becomes a group with `.ease()` / `.over()`.

```rust
let animation = scene.compile([
    square.shift(-80.0, 0.0).spring(spring),
    square.fill_to(ORANGE).over(ms(300)), // colors tween; springs would overshoot
    all([square.scale_by(0.3).spring(spring), square.rotate(0.4).spring(spring)]),
]);
```

Procedural code builds the tree with ordinary Rust. Insertion sort, where the
algorithm mutates `order` while describing each pass:

```rust
scene.defaults(ms(400), ease::cubic::ease_in_out);
let mut passes = Vec::new();
for i in 1..order.len() {
    let mut j = i;
    while j > 0 && order[j - 1].0 > order[j].0 {
        order.swap(j - 1, j);
        j -= 1;
    }
    passes.push(chain([
        status.set_content(format!("INSERTION PASS {i} / {}", order.len() - 1)),
        all(order.iter().enumerate().map(|(index, (_, bar))| {
            bar.position_x().to(slot(index, order.len()))
        })),
    ]));
}
let mut steps = passes;
steps.push(status.set_content("SORTED / HOLD"));
steps.push(wait(ms(600)));
let animation = scene.compile(steps);
```

Reusable effects are functions returning `Anim`:

```rust
fn pop(dot: CircleHandle) -> Anim {
    chain([dot.scale().to(1.2).over(ms(150)), dot.scale().to(1.0)])
}
flow(ms(120), commits.iter().map(|commit| pop(commit.dot)))
```

## 3. Markers and looping

Today `lissajous` returns a `Duration` out of the authoring closure to configure
`PlaybackMode::LoopFrom(loop_start)`. With markers:

```rust
let animation = scene.compile([
    intro,                 // grid reveal, plays once
    marker("loop"),
    diagonal_waves,        // repeats
]);
Window::new("Lissajous", CANVAS)
    .playback_mode(PlaybackMode::LoopFrom(animation.marker("loop").unwrap()))
    .run(animation)
```

A marker's time comes from its position: in a `chain`, where the previous step
ends; in `all`, the group start; in `flow`, its slot start. `compile` records
each marker's final time after core lays out the timeline. Names are plain
strings: `animation.marker(name)` returns `Option<Duration>`, and `compile`
reports a duplicated name as an error. Typed marker handles can be added later
if string typos become a real problem.

The animation describes content and named times; the host decides how to loop,
consistent with keeping playback controls out of animation descriptions.
Markers also help later with editor timelines, audio sync and scripts
("jump to `chapter-2`").

Infinite repetition can't be baked (it has no finite duration), so it stays a
host playback mode. Finite repetition uses `.repeat(n)`. Core's multi-track timelines
(chapters) are an alternative to markers; markers are simpler and work at any
time, so they come first.

## 4. Objects

### Why handles stay

To animate an object you need a reference to it. In Rust, the alternatives are:

| Option | Typed | Cost | Notes |
|---|---|---|---|
| Typed handle (current `CircleHandle`, a `Copy` ID) | yes | free; dense indices | needs an ID allocator at creation |
| String names | no | hash lookup | runtime typos; good for scripts |
| Global ID counter, no scene needed | yes | world must map ID to slot by hash | hidden process-global state |

Typed handles are the best fit for Rust. Scripts and documents refer to objects
by name and are mapped to handles when loaded. The `&mut Scene` requirement
comes from the ID allocator, but it can be hidden behind value-based
composition.

### Adding several objects at once

`scene.add` accepts one object, an array, a `Vec` or any iterator of one object
type. A single blanket impl (`impl<I: IntoIterator> SceneObject for I where
I::Item: SceneObject`) covers every collection and returns a `Vec` of handles;
verified to compile across crates, as long as no separate array/`Vec` impls
exist and no object type implements `IntoIterator`.

```rust
scene.add([
    Text::new("Insertion sort").position((-400.0, 260.0)).font_size(21.0).fill(PASTEL_BLUE),
    Text::new(format!("INPUT  {input}")).position((-400.0, 224.0)).font_size(14.0),
]);
scene.add((0..n).map(|i| Circle::new(4.0).position((i as f64 * 20.0, 0.0))));
```

Mixed types in one collection would need tuple impls or an erased object type,
so mixed objects use separate calls or `.child(..)` nesting.

### Why not implicit adding, a macro, or refs

- **Implicit adding** (constructors register into a hidden current scene)
  needs thread-local global state, puts objects on screen when you only wanted a
  description, and hides ownership.
- **A `scene! { .. }` macro** that adds bare expressions saves only the
  `scene.add(` text. `macro_rules!` can't see types, so it needs grammar rules
  (comma means "add"; `let`, `for`, `if`, `match`, blocks and closures each
  special-cased), plus new syntax to capture handles (`balls.push(Circle..)`
  can't both add and return). The body isn't valid Rust syntax, so rustfmt
  won't format it; rust-analyzer completion degrades; typos give "no rules
  expected the token" errors; and a `;` typed instead of `,` silently drops an
  object. Arrays, iterators and `.map(..).collect()` for handles get most of
  the readability in plain Rust.
- **Proc-macro DSLs** (Dioxus `rsx!`, Leptos `view!`, Yew `html!`) are the
  full-scale version: workable, but each needs a parser, error reporting and
  its own formatter (Dioxus ships `dx fmt`). Only worth considering if
  MotionGfx grows a layout-heavy declarative object tree, as its own project.
- **Motion Canvas refs** exist because a JSX tree is one expression; in Rust,
  `let handle = scene.add(..)` is already a typed ref.

**Adding objects "during" the animation:** steps are built outside `compile`
while `scene` is still available, so creation can sit where it happens in the
script (`let extra = scene.add(..opacity(0.0)); steps.push(extra.opacity().to(1.0))`).
Every object still exists from time zero, so seeking works. Motion Canvas's
`insert` additionally animates layout reflow; MotionGfx has no layout system
yet, and that is a separate future feature that would build on `Group` and
`.children`.

### Children: `.child` and `.child_of`

- **`.child(x)`** adds a child inside a builder; use it for objects you never
  reference again. `.children(iter)` adds many of one type; `.child` is generic
  per call, so mixed types chain (`.child(text).child(line)`).
- **`.child_of(parent)`** attaches an object to an existing parent's handle; use
  it when the child needs its own handle. It replaces today's `relative_to`
  (Bevy's equivalent is the `ChildOf` relationship). The name `relative_to` can
  later mean a narrower position-only constraint.

```rust
let bar = scene.add(
    Rectangle::new(64.0, height)
        .position((slot(index, n), height / 2.0 - 200.0))
        .radius(8.0)
        .fill(PASTEL_BLUE)
        .child(Text::new(value.to_string()).position((0.0, -height / 2.0 - 12.0))),
);
let marker = scene.add(Circle::new(4.0).position((0.0, 20.0)).child_of(bar));
```

**Children are an ordered tree.** Each parent keeps an ordered child list
(code order, with an optional `.index(i)`), not only a parent pointer, because
layout needs order. Decide this now; it's costly to retrofit.

**Inheritance (standard scene-graph model):**
- *Transform:* world transform = parent transform x local transform (what
  `relative_to` does today).
- *Opacity:* multiplied down the tree, rendered as a Vello `push_layer` with
  alpha around a faded subtree, so a fading group composites as one image
  (multiplying per-object alpha is cheaper but lets overlapping children show
  through each other).
- *Styles:* shared explicitly, not inherited. Style values (`.style(&label)`) and
  signals already share fills, fonts and colours across objects without
  per-property inheritance rules.

**`Group` (optional):** an object with no visuals, only a transform and opacity
for children: local coordinates, moving or fading many objects with one
animation, section roots. Layout containers (below) are its layout-aware sibling.

### Layout (`layout` feature of `motiongfx_2d`)

Motion Canvas-style layout animation (`gap`, `grow`, `padding`, insert/remove
with reflow) fits the redesign without changing `Anim`:

- **Layout properties are ordinary animatable properties**: `group.gap().to(56.0)`
  is a leaf; timelines, seeking and markers work unchanged.
- **Layout owns its children's position and size.** Animate layout properties,
  not child positions. `layout(false)` opts a child out.
- **Per-frame order:** sample timeline, then computed values and bindings, then
  layout (writing positions and sizes), then render. Authoring's existing
  post-sample hook (`NativeUpdate`) is where the layout pass registers. Layout
  is derived from sampled values, so seeking stays exact.
- **Recompute only when needed.** Layout is a pure function of layout
  properties, tree structure and measured content sizes. Each frame, compare
  layout inputs with their previous values; changed nodes update their Taffy
  style and are marked dirty, and Taffy's per-node cache recomputes only what
  they affect. Unchanged frames do no layout work. The compiled timeline also
  knows when layout properties have clips, so spans with no animated layout
  input (and no computed value feeding one) can skip even the comparisons.
  Reliability rule: every write to a layout input goes through tracked paths
  (timeline sampling, computed values and bindings); direct `NativeUpdate`
  callbacks must not change layout inputs without marking nodes dirty.
- **Slot vs drawing: `layout_scale`.** As in CSS, transforms are visual only: a
  child scaled to 0 keeps its layout box. Animating `width` changes both the
  slot and the drawing (a sliver widening), and text, circles and paths don't
  resize sensibly. `layout_scale` (0 to 1) scales only the item's footprint in
  the layout (size, and ideally its share of the gap) while it draws at its real
  size; combine it with visual `scale` for a pop-in:
  `all([extra.layout_scale().to(1.0), extra.scale().to(1.0)])`. Both are
  ordinary animatable properties. (Without `layout_scale`, a `Group` slot with
  animated width and a centred, scaling `layout(false)` child works, at three
  objects per insert.)
- **Gaps.** `gap` itself is an ordinary animatable style value. The only subtlety
  is insertion: flexbox puts a gap between every pair of items, including
  zero-size ones, so a newly joined item would add one full gap instantly.
  While an item's `layout_scale` is `s < 1`, the layout crate gives it a margin
  of `-(1 - s) * gap` on one side (trailing for the first item; top/bottom in
  columns). At `s = 0` the layout is unchanged; at `s = 1` it's normal. Taffy
  supports negative margins, and users never see this margin.
- **Handles to nested nodes** come from `let` plus `.child_of`, the Rust
  equivalent of Motion Canvas refs.

```rust
let group = scene.add(
    Layout::row().gap(20.0).padding(20.0)
        .children(palette.map(|fill| Rectangle::new(90.0, 120.0).radius(12.0).fill(fill))),
);
let extra = scene.add(
    Rectangle::new(90.0, 120.0).radius(12.0).fill(BLUE)
        .child_of(group).index(1).layout_scale(0.0).scale(0.0),
);
scene.compile([
    wait(ms(300)),
    all([extra.layout_scale().to(1.0), extra.scale().to(1.0)]).over(ms(600)),
    wait(ms(200)),
    group.gap().to(56.0).over(ms(600)),
    wait(ms(200)),
    all([extra.layout_scale().to(0.0), extra.scale().to(0.0)]).over(ms(600)),
    group.gap().to(20.0).over(ms(600)),
    wait(ms(400)),
])
```

```rust
let root = scene.add(Layout::row().gap(10.0).padding(10.0).size(440.0, 240.0));
let col_a = scene.add(Rectangle::flex().grow(1.0).fill(DARK).radius(4.0).child_of(root));
let column = scene.add(Layout::column().gap(10.0).grow(3.0).child_of(root));
let row_a = scene.add(
    Rectangle::flex().grow(8.0).fill(RED).radius(4.0).stroke(WHITE, 4.0).margin(2.0).child_of(column)
        .child(Circle::new(10.0).fill(WHITE).layout(false)),
);
scene.add(Rectangle::flex().grow(2.0).fill(DARK).radius(4.0).child_of(column));
let col_b = scene.add(Rectangle::flex().grow(3.0).fill(DARK).radius(4.0).child_of(root));

scene.compile([
    all([col_b.grow().to(1.0), col_a.grow().to(2.0)]).over(ms(800)),
    row_a.grow().to(1.0).over(ms(800)),
    all([col_b.grow().to(3.0), col_a.grow().to(1.0)]).over(ms(800)),
    row_a.grow().to(8.0).over(ms(800)),
])
```

(Names such as `Rectangle::flex` and `index` are illustrative.)

**Taffy over Morphorm:** Taffy implements CSS flexbox and grid, matching Motion
Canvas's flexbox model (`grow`, `gap`, `padding`, `direction`) and web
knowledge; it's widely used (Bevy UI, Dioxus native), caches per node, and
supports measure functions for text sizes from `motiongfx_text`. Morphorm is
smaller but uses its own model.

**Crate placement: inside `motiongfx_2d`, behind a `layout` feature** (Taffy as
an optional dependency). Layout interacts with almost everything the 2D crate
owns: node positions and sizes, child order, inheritance, text measurement
(already wired to `motiongfx_text`), and the "layout owns child position" rule.
One crate keeps one source of truth for the tree and those rules, and builder
methods (`.grow()`, `.gap()`) are plain methods rather than extension traits. A
separate `motiongfx_layout` crate stays viable with `NodeId` as a uniform key;
choose it only if non-2D models need layout or compile-time isolation matters.
`motiongfx_authoring` stays layout-agnostic. Placing whole scenes into layout
regions belongs to the later `Project` layer.

### Node tree in `motiongfx_2d` (phase 2)

Today `Graphics` keeps a `Vec` per shape type plus a flat `order: Vec<DrawObject>`.
Common properties (`position`, `scale`, `rotation`) are duplicated in every shape
type, there is no uniform object key (`Parent` covers only rectangles and
circles), and draw order is separate from parenting.

The rework splits every object into a node (shared data) and a payload (shape
data):

```rust
struct Node {
    position: Point, rotation: f64, scale: f64, opacity: f64,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,   // children in sibling order:
    next_sibling: Option<NodeId>,  //   draw order and layout order
    kind: NodeKind,                // Rectangle(idx) | Circle(idx) | Text(idx) | .. | Group
    layout: Option<LayoutIndex>,   // into a dense Vec<LayoutProps>
}
struct Rectangle { size: Size, radius: f64, fill: Color, stroke: Color, /* .. */ }
```

- Shared properties live once; cross-cutting features (inheritance, children,
  layout) are added once, not per shape type.
- Every object has a `NodeId`, so any object can be a parent and `Parent`
  disappears. Handles stay typed (`RectangleHandle` wraps a `NodeId`).
- `Group` is a node without a payload.
- Draw order is tree order: the renderer walks the tree, which is what
  inherited transforms and group-opacity layers need. The flat `order` list
  goes away.
- Layout data sits on the node, because layout is a property of the scene tree
  like transform and opacity.
- **No ECS**, even with a node tree. The tree has a fixed shape (node record,
  one payload, optional layout) and a few fixed passes (sample, bindings,
  layout, transforms, render), so there's nothing for ECS queries or scheduling
  to solve, and hierarchies are an awkward ECS fit. `bevy_motiongfx` covers ECS
  users, and core's `SubjectSource` keeps an ECS-backed world possible later.
- **Data layout choices:**
  - *Index links, not per-node child `Vec`s:* `parent`, `first_child`,
    `next_sibling` (as `indextree` does). Fixed-size nodes, no per-parent heap
    allocation, order is sibling order; inserting at an index walks siblings
    at setup time.
  - *Hot/cold split:* transform and opacity stay compact in the node; layout
    properties live in a dense `Vec<LayoutProps>` the node indexes into
    (`layout: Option<LayoutIndex>`).
  - *World-transform cache:* one top-down pass per frame fills a `Vec<Affine>`,
    recomputing only dirty subtrees; renderer and layout read the cache.
  - *Instancing for huge counts:* very large numbers of similar objects
    (particles) become one instanced node with arrays of per-instance data,
    not many nodes. Add only if a use case appears.

| Crate | Role in the rework |
|---|---|
| `motiongfx` (core) | unchanged |
| `motiongfx_authoring` | `Anim`, `compile`, signals; model-agnostic, knows nothing about nodes |
| `motiongfx_2d` | node tree, shape payloads, children, inheritance; `layout` feature (Taffy) |
| `motiongfx_text` | unchanged; text measurement for layout |
| `vello_motiongfx` | tree-walk rendering with opacity layers; per-type caches stay |
| `vello_winit_motiongfx` | unchanged (`Project` later) |

The node tree stays in `motiongfx_2d` because 2D and 3D transforms differ.

Costs: a sizeable rework of `motiongfx_2d` stores, builders and handles, and of
the renderer traversal (the planned children, inheritance and layout work needs
most of it anyway). Property targets split between the node store (shared
properties) and payload stores (shape properties), so a handle must reach both
(a node-to-payload index, or the payload index stored in the handle), building
on the current single-ID handles. Phase 2, after the `Anim` / `compile` change.

### Scope: an engine, not a GUI framework

Scene trees, layout and reactive values overlap structurally with GUI
frameworks, but MotionGfx is time-driven, deterministic and seekable, while a
GUI is event-driven and stateful (input, focus, IME, scrolling, accessibility,
text editing, widgets). MotionGfx stays an engine; a future editor is an
application on an existing GUI toolkit that embeds it. Candidates: Xilem/Masonry
(Linebender, Vello-based, so a MotionGfx `vello::Scene` draws straight into a
widget), egui (wgpu callback or texture), or Bevy (shares Taffy). All share wgpu.
The editor needs from MotionGfx: drawing into a caller-provided `vello::Scene`
(exists), `Animation::sample(time)` for scrubbing, markers, an inspectable `Anim`
tree, and later `motiongfx_scene` documents. Input handling, focus and widgets
stay out of MotionGfx.

### Components

**Default: plain code.** A loop body or a function
(`fn spring_panel(scene: &mut Scene, ..) -> CircleHandle`) is a component. Loops
capture shared values (styles, signals) with no plumbing, and functions return
whichever handles the timeline needs. `spring_curves` already works this way:
its loop body is one panel, and the tuple array supplies the three variations.

**Optional: props structs implementing `SceneObject`** (like egui's
`ui.add(widget)`). These cost a struct, a destructure and a trait impl, with each
prop written several times, so they only pay off for composites reused across
files or shipped by a library (an axis or legend type in `motiongfx_2d`), or
with many optional settings worth naming. Not the default for example code.

**Where the redesign helps `spring_curves`:** the animation, not the objects.
Nested `parallel` callbacks become flat steps, and shared timing is written once
per group:

```rust
let mut steps = Vec::new();
for (index, experiment) in experiments.into_iter().enumerate() {
    /* springs, duration */
    steps.push(title.set(experiment.name));
    if index > 0 {
        steps.push(all([
            physical.to(experiment.physical),
            visual.to(experiment.visual),
            timed.to(experiment.timed),
        ]).over(ms(1000)));
    }
    steps.push(wait(ms(500)));
    steps.push(all(
        balls.iter().zip(springs).map(|(ball, spring)| ball.position_x().to(TRAVEL).spring(spring))
            .chain([elapsed.to(duration.as_secs_f64()).over(duration).ease(ease::linear)]),
    ));
    steps.push(wait(ms(100)));
    // Rewind along the current curves before morphing to new settings.
    steps.push(all(
        balls.iter().map(|ball| ball.position_x().to(-TRAVEL)).chain([elapsed.to(0.0)]),
    ).over(ms(500)).ease(ease::cubic::ease_in_out));
}
scene.compile(steps)
```

### Entering and leaving

No built-in entrance/exit helpers for now. Visibility is ordinary properties
(opacity, reveal, scale) with explicit start states at creation, and reusable
effects are plain functions returning `Anim`:

```rust
fn fade_in(object: RectangleHandle) -> Anim {
    object.opacity().to(1.0)
}

let square = scene.add(Rectangle::new(160.0, 160.0).fill(BLUE).opacity(0.0));
scene.compile([fade_in(square).over(ms(500)), square.shift(-80.0, 0.0)]);
```

Edge cases (fading in a visible object, overlapping opacity animations) follow
normal property rules and core conflict resolution. Built-in helpers, or a
small "has opacity" trait so one helper works for all shapes, can be added later
if they prove useful.

**Removal is not deallocation.** Backward seeking must restore any earlier
state, so every object exists in the world for the whole animation. The
renderer skips fully transparent objects. With opacity inheritance (see
`.child`), hiding a parent hides its children.

### Concurrent and continuous animation

Core has one timeline organised per field (subject + field): each has its own
timed clips. Animations on different fields never interfere; overlapping
changes to the same field are resolved by core's conflict rules at compile.

Something that runs "constantly" alongside the sequence has two forms:

- **A function of time** (runs for the whole animation, survives looping and
  seeking, costs one computed value per frame). The timeline must not also
  animate the same field; nest the object under a parent if both are needed.
  ```rust
  let time = scene.time();
  scene.add(Rectangle::new(80.0, 80.0)
      .rotation(computed!((time) => time.as_secs_f64() * 1.5)));
  ```
- **A timeline branch matching the sequence's length**, using a
  duration-before-compile helper:
  ```rust
  let main = chain(main_steps);
  let length = scene.duration_of(&main);
  scene.compile([all([main, square.rotate(TAU * 4.0).over(length).ease(ease::linear)])]);
  ```

Baking precomputes clip start and end values; sampling still interpolates every
frame, so baking removes closure calls and dependency checks, not evaluation.
Core interpolation and easing are function pointers, so clips can't capture user
closures. Time functions fall into three tiers:

1. **An ordinary tween** (constant speed over a known length): fully baked.
2. **Curve data**, following the spring pattern (a clip stores curve parameters
   evaluated from progress): oscillations, constant velocity. Needs a core
   extension; same per-frame cost as any tween; serializable.
3. **Arbitrary closures**: per-frame evaluation through bindings (today), or a
   core clip holding a boxed closure (skips dependency checks, not serializable).

Prefer 1 or 2 for effects placed in time; use time-driven computed values for
inherently global effects (readouts, ambient motion with no start or end).

### Sections and transitions

One `Scene` compiles to one `Animation`, and the host plays one animation.
"Scenes" in the Manim sense are sections of one scene: a parent object whose
content is its children, plus a function returning its `Anim`. With opacity
inheritance, a transition is ordinary animation:

```rust
scene.compile([
    intro_steps(),
    all([intro.opacity().to(0.0), chart.opacity().to(1.0)]), // crossfade
    marker("chart"),
    chart_steps(),
]);
```

Section roots can be `Group` objects (see "Child nesting").

### Later: a project of scenes

For long videos, per-frame cost should scale with the scene being viewed. The
model is a video editor's: a `Scene` is a composition (its own world, objects,
signals and timeline, compiled to an `Animation`), and a `Project` places scene
clips on a project timeline with transitions between them, like Motion Canvas's
project and scenes.

```rust
let project = Project::new()
    .scene(intro_scene())                                  // each a compiled Animation
    .scene(chart_scene()).transition(Slide::left(ms(600)))
    .scene(outro_scene()).transition(Fade::over(ms(400)));
Window::new("My video", (1920.0, 1080.0)).run(project);
```

- **Time mapping:** project time maps to (active scene, local time); only that
  scene is sampled, evaluated and rendered. Inactive scenes cost nothing per
  frame. Arbitrary seeking already works per `Animation`.
- **Transitions keep the outgoing scene alive until it's out of view.** The
  transition occupies the start of the incoming scene B, while the outgoing
  scene A keeps being sampled past its end: its timeline holds final values,
  and time-driven values (a spinning cube from `computed!((time) => ..)`) keep
  moving. This needs bindings to receive the unclamped local time during a
  transition; today `Animation::sample` clamps `world.time` to the timeline
  duration. The project's length stays the sum of scene lengths. A true overlap
  (the transition starting before A ends, like a cross-dissolve) can be a later
  per-transition option.
- **Scrubbing** across scenes costs about the same as a large seek within one
  scene. The risk is cold renderer caches on the first frame drawn for a scene;
  keep caches for the current and neighbouring scenes, and prewarm the next
  scene during playback. Frames inside a transition draw two scenes.
- **Vello composition:** `Renderer::render` already draws into a caller-provided
  `vello::Scene`. Render each scene into its own `vello::Scene`, then compose:
  `append(&scene, Some(transform))` for slides and zooms, `push_layer` with
  alpha for fades. No textures or extra passes.

| Area | Change |
|---|---|
| Authoring redesign | none |
| Core MotionGfx | none: each scene has its own `Timeline`, `World`, `Registry` |
| New `Project` type | scenes, start times, transitions, time mapping |
| Renderer | one `Renderer` per scene (caches are indexed by object position) |
| Backgrounds | drawn as a full-canvas rect inside each scene, so transitions show both |
| Host | `Window::run` accepts anything playable; `Project` implements the existing `VelloDemo` trait like `AnimationDemo` |
| Markers / looping | project marker = scene start + scene marker; `LoopFrom` uses project time |
| `Animation::sample` | bindings can receive unclamped local time (scene running past its end) |

Trade-offs: scenes share nothing (persistent elements are recreated or placed
on a later project overlay layer), and the simple version compiles all scenes up
front (setup time and memory, not per-frame cost). Lazy compilation from scene
builders and dropping inactive renderer caches are later improvements. Sections
remain for continuity within a scene.

## 5. Cost model

Work happens in two places:

- **Setup** (once): build the tree, register actions, order clips, compile, bake.
- **Playback** (every frame): find active clips, interpolate baked values, evaluate
  bindings, render.

| | Current authoring | `Anim` tree |
|---|---|---|
| Playback | baseline | **identical**: the same core tracks are produced |
| Setup allocations | core's own (boxed actions, fragment maps) plus a temporary `Vec` per callback group | the same, plus the tree: up to one small allocation per leaf (its value) and one `Vec` per group |
| Setup dispatch | direct (generic) calls | one indirect call per leaf |
| Compile time / binary | one generated copy per change shape (monomorphisation) | likely fewer copies |

The extra allocations exist because the tree is stored until `compile`. A
current change is lowered the moment it is played, so its typed value never
needs a heap home. An `Anim` leaf must share one type with every other leaf, so
its variable-sized value lives behind a pointer until lowering. Small common
values could be stored inline to avoid even that, if measurement shows it
matters.

The extra setup cost is estimated at microseconds per thousand changes,
next to setup work core already does. **Measure before claiming any of this**
(see Measurements). The justification is readability and composability, not
binary size.

A fully zero-cost declarative tree is only possible when the compiler sees every
type statically (tuples or generics), which rules out runtime-built `Vec`s,
scripts and documents. That trade-off is inherent, not specific to MotionGfx.

### Rendering performance (related, separate from the redesign)

Current per-frame behaviour: the host calls `scene.reset()` and the renderer
re-records Vello commands for every object each frame. Expensive preparation is
already cached across frames (text shaping per [text-caching.md](text-caching.md),
`PathGeometry`, `CachedTriangle`), so what repeats is command recording. Vello
keeps no GPU state between frames, so caching encoded fragments would save CPU
encoding only, and `Scene::append` still copies fragments.

The concrete waste: `vello_winit_motiongfx` handles `RedrawRequested` by
rendering and immediately calling `request_redraw()`, so it renders at display
rate while paused, during static holds and while idle, producing identical
frames (architecture.md lists idle redraws as a future option).

Priorities:
1. **Stop redrawing when paused or idle.** Request redraws only while playing,
   scrubbing or using controls, on resize or scale changes, and on exposure.
   Small host change; likely the largest practical saving.
2. **Measure before caching encodings.** Per-frame timing split into sampling,
   computed values and bindings, Vello encoding, and GPU time, on a stress scene
   (for example 1k / 10k / 50k objects).
3. **Cache per-object encodings only if encoding is a bottleneck.** The node
   tree's dirty flags would make it straightforward later.

## 6. Scripting, hot reload and `motiongfx_scene`

**Rust hot reload needs no document format.** Setup is cheap, so a reload can
rebuild the scene and timeline and restore the playback time (file watcher,
reloadable library, or a hot-patching tool).

**Scripts, saved documents and node graphs need data.** The plan is to convert
the `Anim` tree to and from `motiongfx_scene` rather than invent a second format.
`motiongfx_scene` today has `Chain`/`All`/`Flow` blocks, action leaves, seeds
and value pools, resolved by name through a registry. It would need:

| Addition | Notes |
|---|---|
| `Any`, springs, group defaults, markers | mirror `Anim` |
| Object kinds and creation | subjects gain a kind name; constructors registered by name |
| Parenting | ordered child lists (`.child` / `.child_of`) |
| Signals | subjects with one field (seed plus actions) |
| Computed values and reactive builder inputs | a data graph of ops and named native functions, bound to fields |
| Text content and selection changes | registered ops |

**Ownership:** `motiongfx_scene` stays generic and knows nothing about 2D.
`motiongfx_authoring` owns `Anim`, signals, the evaluator and the conversion.
`motiongfx_2d` registers its object kinds, fields, ops and value types, the
same way the scene README registers fields and ops today.

### Computed values without per-frame script cost

The current Rust evaluator is already a dependency graph: order is prepared
once, and each frame it compares inputs and reruns changed closures. Scripts
would use the **same evaluator with a second node kind**:

- native nodes: Rust closures from `computed!` / `.map` (today's behaviour), and
- data nodes: built-in ops (`add`, `mul`, `sin`, ..) and named native functions
  registered from Rust (`"lissajous_point"`), so scripts call compiled Rust
  rather than running script code per frame.

Data graphs are compiled once into a flat, ordered instruction list, and pure
arithmetic chains can be merged into one node, so per-frame cost stays near
nanoseconds per op. A Rust-only scene contains only native nodes and pays
nothing for the interpreter. Script loops that build timelines run once at
setup, like Rust's.

## 7. Naming

`motiongfx_authoring::Scene` (live construction state) and
`motiongfx_scene::Scene` (saved document) share a name. Rust paths and
`use .. as ..` resolve this wherever both are imported. If one is renamed, it
should be the document type (for example `SceneDocument`), since it's a saved
description rather than a live scene. `Scene` is the semantically right name
for authoring, and matches Manim.

## 8. Removed and migrated APIs

Removed: `Sequence` as a user type, `animate` (renamed `compile`), `play`, `play_all`, `wait` (method),
`parallel` / `chain` / `flow` callbacks, `defaults` on the sequence (moves to
the scene), `animate!`, returning durations for looping.

To migrate:
- `AnimationChange` impls in `motiongfx_authoring` (changes, signals, springs) and
  `motiongfx_2d/src/text_selection.rs`.
- Examples using `animate`/`compile`: `bezier_explainer`, `camera_zoom`, `commit_graph`,
  `insertion_sort`, `lissajous`, `moving_square`, `radius_area`,
  `spring_curves`, `text_animation`.
- Tests in `motiongfx_2d/tests`, `motiongfx_authoring/tests` and
  `vello_motiongfx/src/*_tests.rs`.
- READMEs and architecture.md.

`*_low_level` examples use core directly and are unaffected.

## 9. Decisions and remaining questions

Decided:
- Leaves are data-first with a `Custom` escape hatch.
- Markers use string names.
- Defaults: library defaults, overridable with `scene.defaults(..)`, then group
  `.ease()` / `.over()`, then per-leaf values.
- `anim.repeat(n)` plays `anim` n times in a row, as separate passes (a
  one-line wrapper over `chain(std::iter::repeat_n(anim, n))`). Each pass
  starts from where the previous one ended: `offset(10).repeat(3)` is three
  moves of 10 (30 in total over three durations); `offset(30)` is one move.
  Absolute ops (`to`) only change on the first pass.
- No built-in entrance/exit helpers for now (see "Entering and leaving").
- `scene.duration_of(&anim)` gives a tree's length before compiling, using scene
  defaults; markers give exact times after.
- Children: `.child(x)` inside builders, `.child_of(parent)` for children with
  their own handles (replacing `relative_to`); ordered child lists; transform
  and opacity inherited, styles shared explicitly (see "Children").
- `compile` consumes the scene; hooks stay `FnOnce`.
- Layout uses Taffy as a `layout` feature of `motiongfx_2d`, on a node tree
  (see "Layout" and "Node tree").

Deferred (separate features, no architectural impact):
- Partial inheritance constraints, such as a child that follows only its
  parent's position.
- The layout implementation itself (after the node tree).
- A `Project` of scenes with transitions (see "Sections and transitions").
- Curve-data clips for oscillation and constant velocity (core extension).

## 10. Measurements to take

Before and after, on a large generated scene (for example, 10k leaves), in
release mode:

- setup time of `scene.compile` (tree build, lowering, compile, bake),
- per-frame sample time (expected unchanged),
- `cargo build` time and example binary size.
