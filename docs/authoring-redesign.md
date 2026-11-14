# Authoring redesign: animations as values

Status (2026-11-15): **phase 1 decided, not implemented.** Phase 2 (2D node
tree) is a reviewed proposal. Layout and the other "later" items are deferred.
[architecture.md](architecture.md) records the current implementation; update
it as each step lands.

## Goals

- Animation code that reads like Manim or Motion Canvas: describe objects,
  then describe what happens to them, with little plumbing.
- Composable: animations are ordinary values that functions build, return,
  store, clone and combine. Loops, iterators, `if` and `match` are plain Rust.
- Plain Rust and good tooling: no per-arity trait machinery, no DSL macros.
- Keep core MotionGfx (timeline, baking, arbitrary seeking) as the foundation.
  The new layer is a description that compiles to core; it has no scheduling
  rules of its own.
- A small API. Add features when an example needs them, not in advance.

## Phase 1 decisions

1. **One animation type, `Anim`.** Property changes, signal changes, text
   selection changes, waits, markers and groups are all `Anim`, so arrays,
   `Vec`s, iterators and `if`/`match` branches mix them freely.
2. **Groups:** `chain`, `all`, `flow(interval, ..)` take any
   `IntoIterator<Item = Anim>`. Steps: `wait(d)`, `marker("name")`.
3. **Modifiers on any `Anim`:** `.over(d)`, `.ease(e)`, `.spring(s)`,
   `.delay(d)`. Groups have no timing settings of their own (see Timing).
4. **Defaults:** `scene.defaults(duration, ease)`, once per scene, then the
   library defaults (1 s, `smooth`).
5. **`scene.compile(steps)` / `scene.try_compile(steps)`** replace
   `animate` / `try_animate`. The top-level list runs in order (an implicit
   `chain`). `compile` consumes the scene.
6. **Each step is a small trait object** (`Step`) that adds itself to the core
   timeline during `compile`. Three implementations: object property, signal,
   text selection. This replaces `AnimationChange` and the eight change types.
7. **Springs work on any interpolated property.** Each value type defines its
   overshoot behaviour; only discrete properties cannot spring.
8. **Markers live in core** and are shifted by the same combinator code as
   clips. `animation.marker(name)` returns their time.
9. **No `Sequence`, callbacks or `animate!`.** See "Removed APIs".

```rust
let mut scene = Scene::new();
let square = scene.add(Rectangle::new(160.0, 160.0).fill(BLUE));
let spring = Spring::timed(ms(650)).bounce(0.65);

let animation = scene.compile([
    square.shift(-80.0, 0.0).spring(spring),
    square.fill_to(ORANGE).over(spring.duration()),
    all([square.scale_by(0.3).spring(spring), square.rotate(0.4).spring(spring)]),
]);
```

## 1. `Anim`

```rust
// motiongfx_authoring
pub struct Anim<M>(Node<M>);

enum Node<M> {
    Step(Arc<dyn Step<M>>, Timing),       // one change
    Group(Order, Vec<Anim<M>>),           // chain / all / flow
    Wait(Duration),
    Marker(String, &'static Location<'static>),
}
enum Order { Chain, All, Flow(Duration) }

struct Timing {
    over: Option<Duration>,
    ease: Option<EaseFn>,
    spring: Option<Spring>,
    delay: Duration,
    location: &'static Location<'static>, // file and line, for errors
}

/// Adds one change to the core timeline during `compile`.
trait Step<M>: Send + Sync {
    fn lower(&self, cx: &mut Lowering<M>, timing: Resolved)
        -> Result<TrackFragment, SceneError>;
}
```

- **One type because lists need one type.** Each current change is its own
  Rust type, so a mixed array cannot hold them. Storing each step behind a
  pointer to a trait object (`Vec<Box<dyn Trait>>` style) is the standard Rust
  answer; core already does it for actions (`Box<dyn Action<T>>`).
- **Typed until erased.** `square.position_x().to(1.0)` is still checked by
  Rust (field exists, value type matches). Erasure happens inside the
  constructor; users never write `Box` or `.into()`.
- **Generic over the model, hidden by an alias.** Turning a change into core
  actions needs the world type, and 2D handles work with any `Model2d`, so the
  type is `Anim<M>`. `motiongfx_2d` exports `pub type Anim = Anim<Graphics>`,
  the same pattern as `Scene`. User code writes `Anim`.
- **`Arc` steps, `Vec` children.** `Arc` makes `.clone()` a reference-count
  increment, so reusing an animation is cheap. Children come from `collect()`;
  `Arc<[T]>` would only add a copy. Timing is stored inline.
- **Targets resolve during `compile`.** Signals get their storage slot and
  text selections resolve against the text content when the step is lowered,
  so both can be described before or without the scene.
- **Spring is a timing setting, not a wrapper type.** Target kinds and timing
  options add up (three `Step` implementations plus one `Timing`) instead of
  multiplying into one type per combination. A new option such as `.delay()`
  is one field and works for every target kind and for groups.

### `Anim` versus core `TrackFragment`

| | `Anim` | `TrackFragment` |
|---|---|---|
| Created | anywhere, while writing the script | through a `TimelineBuilder` (`&mut Registry`) |
| Timing | may use the scene default | fixed at creation |
| Targets | may be unresolved (signals, selections) | concrete subject IDs |
| Clone / reuse | yes | no (clones would share action IDs) |

`compile` turns `Anim`s into `TrackFragment`s using core's own
`chain`/`all`/`flow`/`delay`. Making core fragments lazy instead was
considered: same speed, one set of combinator names, but core would take on
type erasure and deferred resolution that raw core users do not need. Keep
`Anim` in authoring; moving it into core later is mechanical if wanted.

## 2. Timing

- **Duration:** the animation's own `.over()`, otherwise the scene default.
  **Ease:** its own `.ease()`, otherwise the scene default.
- **Spring:** `.spring(s)` replaces tween timing; the spring's settle duration
  is reserved. Combining `.spring()` with `.over()` or `.ease()` on the same
  animation is an error from `compile`.
- **Delay:** `.delay(d)` on any `Anim` (groups included) starts it `d` later.
  It is `chain([wait(d), anim])`; the delay counts towards the enclosing
  group's length.
- **Instant changes:** `.set(..)`, `set_fill`, `set_content` are always zero
  duration, regardless of defaults.
- **Groups have no timing settings.** A group's length is what its children
  add up to. Share timing with ordinary variables:

  ```rust
  let rewind = ms(500);
  all([
      elapsed.to(0.0).over(rewind),
      ball.position_x().to(-TRAVEL).over(rewind),
  ])
  ```

  If group timing is added later, it means only "default for children that
  have no `.over()`/`.ease()` of their own" (never a total duration), so
  `chain([a, b, c]).over(s(1))` would take 3 s. Not built now.

## 3. Groups and steps

```rust
chain([a, b, c])        // one after another
all([a, b])             // together; the group lasts as long as the longest
flow(ms(50), [a, b, c]) // starts 50 ms apart; lasts until the last one ends
wait(ms(200))           // pause
marker("loop")          // named point in time, zero length
anim.delay(ms(500))     // start this one later
```

- Free functions, not methods on arrays: they accept iterators directly and
  read like Motion Canvas's `all(...)`/`chain(...)`.
- **Bare arrays are not groups.** `[a, b]` could mean "together" (Manim) or
  "in order" (Motion Canvas), so nested groups are always named. Only the
  top-level list given to `compile` is an implicit `chain`.
- **`wait` inside `all` does not delay siblings**: every child of `all` starts
  together. Use `.delay(d)` or `chain([wait(d), anim])`.
- **`for` loops produce no value** (`for` is a statement; Rust has no stable
  `yield`). Collect instead: `steps.push(..)` in a loop, `iter().map(..)`, or
  `iter().flat_map(|x| [a(x), b(x)])` for several animations per item.
- Core's `any` is not exposed: no example uses it.

### Same property in several places

- **Not overlapping, any order:** valid. Parallel branches naturally add a
  later-starting clip before an earlier one; core sorts each property's clips
  at compile (fixed in `7c7e2f4`, previously sampled wrong values).
- **Overlapping:** core currently keeps the later-written clip and drops the
  whole earlier clip, silently. Planned: report it as a `compile` error with
  both locations (core already finds every overlap; its `diagnostics` feature
  records them). Possible later feature: **takeover**, where the earlier clip
  is cut at the overlap and the later one starts from the value at that
  moment. Baking already chains start values, so this is a contained core
  change with no per-frame cost.

## 4. Markers

Looping currently returns a duration out of the authoring closure
(`lissajous`, `bezier_explainer`). Markers replace that and later serve an
editor timeline:

```rust
let animation = scene.compile([intro, marker("loop"), waves]);
Window::new("Lissajous", CANVAS)
    .playback_mode(PlaybackMode::LoopFrom(animation.marker("loop").unwrap()))
    .run(animation)
```

- **In core.** `TrackFragment` carries `(name, time)` markers that
  `chain`/`flow`/`delay` shift exactly like clips, so marker times cannot drift
  from clip times. The compiled timeline exposes them;
  `Animation::marker(name) -> Option<Duration>` reads them.
- **Placement:** in `chain`, where the previous step ends; in `all`, at the
  group start; in `flow`, a marker does not take a slot (it marks where the
  next child starts), so it never delays anything.
- Names are strings. A duplicate name is a `compile` error with file and line.
- Infinite repetition stays a host playback mode (`LoopFrom`); the animation
  only names times.

## 5. Springs

Rule: **anything that interpolates smoothly can spring; each value type
defines what overshoot means.**

- Numbers, points, vectors and sizes extrapolate (overshoot).
- Colours clamp each channel. A future HSL colour type springs per channel
  without special handling.
- Values with valid ranges are clamped where they are used (the renderer
  already clamps opacity).
- Discrete properties (text content, cap/join, any `Property::discrete` or
  `.set`) cannot spring: `compile` reports it with file and line. The `fields!`
  macro already marks discrete fields, so no extra declaration is needed.

Implementation notes:
- The spring step uses the property's stored interpolation function, so core's
  `SpringValue<M>` opt-in bound can relax to plain interpolation.
- Every interpolation must behave sensibly for progress outside `0..=1`.
  Audit: `Color` (clamp), `Spring` settings (used by `spring_curves`, bounded
  same-mode interpolation), geometry/size types.
- No initial velocity or interruption support; unchanged from today.

## 6. Errors

Still compiler errors: wrong field, wrong value type, a non-`Anim` in a list,
wrong argument types.

Reported by `compile` (all collected, not just the first; each with the
`#[track_caller]` file and line of the animation):
- `.spring()` together with `.over()`/`.ease()` on one animation,
- springing a discrete property,
- duplicate marker names,
- invalid spring settings, missing or invalid text selections, binding errors,
- (planned) overlapping clips on one property.

Typestate (one type per timing state, as today) was considered: it keeps these
as compiler errors at no runtime cost, but mixed lists need one type, so it
would cost `.into()` or an `anims![]` macro on every mixed list. Not worth it
for two rare mistakes. A headless test that builds every example's animation
makes these errors appear in `cargo test` for people and agents.

## 7. `scene.compile`

`compile(self, steps)`:
1. runs preparation hooks (`prepare_with`), as today,
2. lowers the tree in source order (registers signals, resolves selections,
   resolves timing),
3. compiles the core timeline, prepares bindings, bakes, samples time zero.

- **Consumes the scene:** the world moves into the `Animation` without
  cloning, and preparation hooks stay `FnOnce`. Variants come from calling a
  scene-building function again.
- Steps are built while `scene` is still available, so objects can be created
  next to the animation that uses them.
- `scene.defaults(..)` is read at `compile`, so it applies to every step,
  including steps built before the call.
- No mid-script "time so far"; markers cover looping.

Procedural code builds steps with ordinary Rust:

```rust
scene.defaults(ms(400), ease::cubic::ease_in_out);
let mut steps = Vec::new();
for i in 1..order.len() {
    let mut j = i;
    while j > 0 && order[j - 1].0 > order[j].0 {
        order.swap(j - 1, j);
        j -= 1;
    }
    steps.push(status.set_content(format!("INSERTION PASS {i} / {}", order.len() - 1)));
    steps.push(all(order.iter().enumerate().map(|(index, (_, bar))| {
        bar.position_x().to(slot(index, order.len()))
    })));
}
steps.push(status.set_content("SORTED / HOLD"));
steps.push(wait(ms(600)));
let animation = scene.compile(steps);
```

Reusable effects are functions returning `Anim`:

```rust
fn pulse(commit: &Commit) -> Anim {
    all([commit.ring.scale().to(2.4), commit.ring.opacity().to(0.0)])
}
let animation = scene.compile([
    trunk.end().to(1.0).over(ms(800)),
    flow(ms(120), commits.iter().map(|commit| {
        commit.dot.scale().to(1.0).over(ms(400)).ease(ease::back::ease_out)
    })),
    fork.end().to(1.0).over(ms(800)),
    all(commits.iter().map(pulse)),
    wait(ms(400)),
]);
```

## 8. Cost model

- **Playback: identical.** `compile` produces the same core timeline.
- **Setup:** about one extra small allocation per step (its `Arc`) plus one
  `Vec` per group, because steps are stored until `compile`. Lowering already
  allocates about 2–3 times per change (boxed core action, the single-entry
  `HashMap` in `TrackFragment::single`). Expected to be small next to core's
  conflict check, which compares every pair of animated properties
  (O(K²)), and baking.
- **Avoiding the extra allocation** (inline storage for small values, or an
  arena with lifetimes) costs `unsafe`/dependencies or composability. Only
  revisit if the benchmark shows the tree.
- **Build time / binary size:** probably similar or slightly smaller (fewer
  generic change types). Measure before claiming.

## 9. Removed APIs and migration

Removed: `Sequence` (public), `animate`/`try_animate` (now
`compile`/`try_compile`), `play`, `play_all`, `wait` (method), the
`parallel`/`chain`/`flow` callbacks, `sequence.defaults` (now on the scene),
`sequence.set`, `append`, `action`, `animate!`, `AnimationChange`, the change
types (`Change`, `SpringTargetChange`, `SpringChange`, `SignalChange`,
`SignalTargetSpringChange`, `SignalSpringChange`, `SelectionChange`,
`SelectionSpringChange`), `spring_to`, `map_target`, returning durations for
looping. `Property::map` / `.map` for computed values is unaffected.

To migrate:
- Examples: `moving_square`, `insertion_sort`, `commit_graph`, `radius_area`,
  `camera_zoom`, `text_animation`, `sine_unit_circle`, `spring_curves`,
  `lissajous`, `bezier_explainer`, and `examples/support/*_tests.rs`.
  `*_low_level` and `timeline_wave` use core directly and are unaffected.
- Crate tests: `motiongfx_authoring/tests`, `motiongfx_2d/tests`,
  `vello_motiongfx/src/*_tests.rs`.
- Doc examples: `motiongfx_2d` crate docs, `tips.rs`, `style.rs`; the two
  `compile_fail` spring doctests in `motiongfx_authoring/src/spring.rs` become
  tests of the `compile` error messages.
- READMEs and `architecture.md`.

## 10. Implementation plan

0. Update this document. *(done)*
1. **Baseline benchmark** (see Measurements) and save a Criterion baseline.
2. **`Anim` + `compile` alongside the current API:** `Anim`, combinators,
   `Timing`, `Step` for property/signal/selection, spring overshoot audit,
   error collection with locations. Unit tests in `motiongfx_authoring` for
   the timing rules, `.delay`, instant changes and both error cases.
3. **Migrate the examples** one at a time (no per-example parity tests; core
   is unchanged and the lowering rules are unit-tested). Existing example
   tests (`lissajous`, `spring_curves`) keep running.
4. **Markers in core**, then migrate `lissajous` and `bezier_explainer`.
5. **Headless test that builds every example's animation.**
6. **Remove the old API** and migrate remaining tests and doc examples.
7. **Docs and after-measurements**; one visual pass over the examples.

Separate, any time: stop the preview host redrawing while idle (see Rendering
performance); overlap errors (easiest after step 6).

## 11. Measurements

Criterion bench `vello_motiongfx/benches/authoring.rs` (same pattern as core's
`action_storage` bench), release mode, 1k and 10k animated circles with
chained, parallel, staggered and spring animations plus signal-driven radii:

| Benchmark | Measures |
|---|---|
| `animate` | authoring + lowering + core compile + bake (`animate` now, `compile` after) |
| `sample_forward` | a forward playback sweep (per-frame path) |
| `seek_backward` | a backward sweep |
| `encode` | the renderer recording one frame into a `vello::Scene` |

Object setup and benchmark IDs stay identical; only the animation function is
ported in step 3. Record with `cargo bench -p vello_motiongfx --bench authoring
-- --save-baseline before`, compare with `-- --baseline before`. Also record
`cargo build --release --timings` for the example crate and example binary
sizes.

### Baseline (before phase 1, 2026-11-15, release, Windows)

| Benchmark | 1k circles | 10k circles |
|---|---|---|
| `setup/objects` | 91 µs | 1.09 ms |
| `setup/animate` | 26.9 ms | **4.86 s** |
| `playback/sample_forward` (121 frames) | 11.2 ms (≈93 µs/frame) | 87.6 ms (≈724 µs/frame) |
| `playback/seek_backward` (121 frames) | 11.0 ms | 89.4 ms |
| `render/encode` (1 frame) | 138 µs | 1.35 ms |

- Release rebuild of the example crate after touching every example: 5.5 s.
- Example binaries: 12.2–12.5 MB (authoring examples); 8.6–8.8 MB
  (`*_low_level`, `timeline_wave`).
- `setup/animate` grows about 180x for 10x the objects, which is quadratic.
  Most likely core's `resolve_conflicts` pairwise check over every animated
  property (about 50k at 10k circles); not yet profiled. It dominates setup
  and would hide the `Anim` tree's own cost at 10k, so compare phase 1 at 1k,
  or fix the conflict check first (group by subject before comparing pairs).
- Per-frame CPU at 10k (sampling + encoding) is about 2 ms, well inside a
  60 fps budget; idle redraws in the host remain the practical waste.

## Phase 2: node tree in `motiongfx_2d` (proposal)

After phase 1. `Anim` code does not change when handles switch to nodes.

**Why:** `Graphics` keeps a `Vec` per shape type plus a flat draw order.
Position/scale/rotation are duplicated per type, and parenting is inconsistent:
`Parent` covers only rectangles and circles; a rectangle parent applies its
scale while the rectangle itself scales size but not stroke, and a circle's
scale includes its stroke; rectangles, lines, paths and triangles ignore
parents when rendered; `Rectangle` has no opacity.

**Shape:** every object is a node (transform, opacity, parent, ordered
children via `first_child`/`next_sibling`) plus a shape payload. `Group` is a
node without a payload. Draw order is tree order. Handles stay typed and store
`{ node: u32, payload: u32 }` (8 bytes, no lookup). No ECS.

- **Children:** `.child(x)` inside builders for objects without handles;
  `.child_of(parent)` for children that need handles (replaces `relative_to`;
  the camera's `relative_to` means "follow" and stays). Ordered child lists
  from the start, because layout needs order later.
- **Inheritance:** transform multiplied down the tree; opacity multiplied, with
  a Vello layer only for nodes that have children and opacity below 1 (leaves
  multiply alpha directly). Styles are shared explicitly, not inherited.
- **World transforms:** recompute all each frame first (one multiply per
  node); dirty tracking only if measured, since every write path would have to
  mark nodes.

Review notes to resolve during phase 2:
- Behaviour changes: rectangle strokes start scaling with the transform;
  inherited opacity changes existing `relative_to` children. Document them.
- `.child(x)` needs type-erased child inserters; store them as clonable `Arc`s
  or `Object: Clone` breaks.
- Bindings run before the world-transform pass, so `.map` on a position reads
  local, not world, coordinates.
- Tree draw order will eventually need z-index.
- Line tips and text selections stay payload-owned, not nodes.
- Layer clip shape: use subtree bounds where known; measure layer cost.

## Later (deferred, not part of this work)

- **Layout** (Taffy, `layout` feature of `motiongfx_2d`), after the node tree
  and only for a concrete example. Open problems found in review:
  - fonts live in the Vello renderer, not the model, so sampling cannot measure
    text yet;
  - bindings run before layout, so a `.map` on a laid-out position would read
    the previous frame's layout and break seeking (layout needs a place in the
    dependency order);
  - write layout results to a separate layout offset, not the animatable
    `position`;
  - Taffy rounds to whole pixels by default; disable rounding for animation;
  - the negative-margin gap idea for `layout_scale` fails with `grow`,
    `space-between/around`, wrapping, grid and automatic minimum sizes;
    `layout_scale` must also scale basis, grow, min-size and padding.
- **Group timing settings** (defaults for children only; see Timing).
- **Takeover** for overlapping clips; spring velocity hand-off later.
- `Project` of scenes with transitions; scripting/hot reload and
  `motiongfx_scene` conversion; computed-value data graphs; curve-data clips
  (oscillation, constant velocity); built-in entrance/exit helpers;
  instancing; an editor (would read `Anim` structure and markers).
- Moving `Anim` into core (lazy fragments), if raw `TrackFragment` authoring is
  no longer needed.

## Objects (independent of phase 1)

- `scene.add` accepts one object, arrays, `Vec`s and iterators through one
  blanket impl (`impl<I: IntoIterator> SceneObject for I where I::Item:
  SceneObject`), returning a `Vec` of handles. Costs: `let [a, b] =
  scene.add([..])` does not work, and a user type cannot implement both
  `IntoIterator` and `SceneObject`. Add `#[diagnostic::on_unimplemented]` to
  `SceneObject` for clear errors.
- Components are plain functions and loops by default; props structs
  implementing `SceneObject` only for widely reused composites.
- Objects exist from time zero (backward seeking must restore any state);
  "removal" is opacity or scale. No built-in entrance/exit helpers yet;
  reusable effects are functions returning `Anim`.

## Rendering performance (separate from the redesign)

- Confirmed: `vello_winit_motiongfx` handles `RedrawRequested` by rendering and
  immediately calling `request_redraw()`, so it renders at display rate while
  paused and idle. With a zero-size window, `render()` returns before
  presenting, so that loop is not throttled by vsync.
- Fix: request redraws only while playing, scrubbing or using controls, on
  resize/scale changes and exposure. Also: let demos request redraws
  (`VelloDemo::window_event` cannot today), redraw for control hover/drag,
  draw one final frame when playback stops at the end, stop on
  `Occluded(true)`, and decide whether the clock pauses while hidden. The event
  loop uses the default wait mode, so it idles once redraws stop.
- Per-frame breakdown to measure on a stress scene (1k/10k/50k objects):
  `queue_actions` (sorts queued fields each frame), sampling,
  `bindings.evaluate` (visits every node), updates, Vello encoding, submission,
  GPU time. Cache per-object encodings only if encoding dominates.

## Naming

- `Anim` (description) and `Animation` (compiled, playable result) sit side by
  side. If that proves confusing, rename the compiled type (for example
  `Playback`), not `Anim`. Decide during migration.
- `motiongfx_authoring::Scene` and `motiongfx_scene::Scene` share a name; if
  one is renamed, rename the document type (for example `SceneDocument`).

## Considered and rejected

- Per-length tuple impls for mixed lists; an `anims![]` conversion macro.
- Typestate change types after erasure (see Errors).
- An object macro or `scene! { .. }` DSL; implicit adding through a global
  current scene.
- A "presence" property (opacity covers visibility).
- Side tables as a general pattern; ECS.
- `compile(&self)` with world cloning.
- `scene.extend` (the blanket `add` covers iterators).
- Props-struct components as the default.
- Group `.over()` as a total duration that rescales children (too many rules).
- Lazy core `TrackFragment`s now (see `Anim` versus `TrackFragment`).
- Exposing `any`, `append`/`action`, `Custom` closure leaves, `duration_of`,
  data-first leaves for future scripting: unused by any example.
