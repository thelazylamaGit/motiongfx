# Interrupting animations

Status: phase 1 (same property, single changes and groups) implemented
2026-11-16. Aliasing paths (phase 2) and spring velocity (phase 3) are
designed below and not implemented.

## Problem

A property can only have one animation running at a time. When a second one
starts before the first ends, core drops the whole earlier clip and authoring
reports `AnimationErrorKind::Overlap`. That is the right default for mistakes
such as `all([x.to(1.0), x.to(2.0)])`, but some scenes mean it: "follow the
latest target". In the sorting example the cursor advances every few
milliseconds and each swap starts sliding as it happens, so a bar that swaps
again must leave its current slide mid-way.

## How other engines behave

| Engine | A new animation on a property that is already animating |
|---|---|
| CSS transitions | New transition from the current value; the old one is dropped. Speed jumps. Reversing to the original value shortens the duration proportionally. |
| GSAP (`overwrite: "auto"`) | Kills the old tween on the overlapping properties; the new one starts from the current value. |
| Framer Motion, react-spring | Springs inherit the current velocity (smooth); tweens restart from the current value. Springs animate each component separately. |
| SwiftUI, Core Animation | Additive: the old animation keeps running as a fading offset and the new one adds on top. Smooth for any curve. |

Three levels of "graceful": continue from the current **value**; also
continue the current **velocity**; or **blend** several animations at once.
MotionGfx does the first now and plans the second for scalar springs.

## Using it

```rust
// One change takes over from whatever `x` is doing when it starts.
x.to(0.0).over(ms(500)).delay(ms(200)).interrupt()
// A group: every change inside may take over.
all(moves).delay(clock).interrupt()
```

Rules:

1. An animation marked `.interrupt()` **cuts** a running animation of the same
   property when it starts. Up to the cut, the earlier one follows its
   **original** timing curve (it does not speed up); after it, it is gone.
2. The interrupting animation starts from the value the earlier one had
   reached at the cut.
3. Only an animation that started **strictly earlier** is cut. Two starting at
   the same instant still overlap (neither is newer), and stay an error.
4. Only the **same property** is cut so far. An interrupter on an aliasing
   path (`position.x` and `position`) is still an `Overlap` error; phase 2
   below.
5. An interrupting animation that overlaps nothing behaves like a normal one.
6. Cutting never changes group timing: `chain`/`all`/`flow` use the reserved
   durations, so the next step of a chain starts when it would have anyway.
7. Interruptions chain: A cut by B, B cut by C, each starting from its
   predecessor's value at the cut.
8. A zero-length interrupter (`.set(..)`) cuts and jumps. A discrete property
   keeps its start value up to the cut, as its jump interpolation already
   does before the end.

`.interrupt()` on a wait or marker returns it unchanged (there is nothing to
cut with). Timing modifiers still work after it:
`x.to(1.0).interrupt().over(ms(100))`.

### Why opt-in

- Overlaps are errors on purpose: they catch real mistakes, reported at the
  user's line. Cutting silently would hide a mis-timed `chain`.
- It states intent on the animation meant to take over.
- Turning it on by default later would not break scenes (scenes that compile
  today have no overlaps); going the other way would.

The `Overlap` message now suggests `.interrupt()` for the later animation.

## Implementation (phase 1)

### Core (`motiongfx`)

- **Marking.** `TrackFragment` has `interrupting: Vec<ActionId>` (empty, so
  no allocation, unless used) and `fn interrupting(self) -> Self`, which marks
  every clip in the fragment. `chain`, `all`, `any` and `flow` merge the lists
  the way they merge markers; `delay` keeps them. A flag on `ActionClip` was
  rejected: measured at 40 bytes, it would pad to 48 on every clip.
- **Cutting.** `TrackFragment::compile` sorts each property's clips by start
  (one pass when already sorted), then `cut_interrupted` walks each sequence
  once: a clip marked interrupting that starts strictly inside the clip
  before it shortens that clip to end where it starts, recording the
  fraction kept. This runs **before** `resolve_conflicts`, so a cut clip no
  longer overlaps and is not reported; everything still overlapping
  (same-instant, aliasing) is resolved and reported as before.
- **Storage.** `Track::cut_progress` holds the kept fraction for each clip in
  the clip arena, by index, or is empty when nothing in the track was cut.
  `Track::cut(span, index)` reads it.
- **Sampling.** `Timeline::queue_actions` already knows each queued clip's
  index. For a cut clip, `cut_mode` maps progress through the shortened clip
  onto the first `kept` of the original (`Interp(t)` becomes
  `Interp(t * kept)`), and its end onto the value at the cut (`End` becomes
  `Interp(kept)`). Ease and spring sampling then work unchanged, so the
  original time mapping is kept for both. The sampling pipeline itself is
  untouched.
- **Baking.** A cut clip stores its full `Segment` (interpolation needs the
  original end) but leaves the value it reached at the cut in the working
  copy, so the interrupter starts there. `value_at` is shared by sampling and
  baking, so they cannot disagree.

### Authoring (`motiongfx_authoring`)

`Animation::interrupt` wraps the animation in a boxed `Node::Interrupt`, which
lowers its inner animation and calls `.interrupting()` on the fragment. The
wrapper works for single changes and groups alike, and adds nothing to the
size of other animations (the 48-byte size test still passes).

### Cost

- **Scenes that don't interrupt:** no new lookups while playing.
  `queue_actions` checks `cut_progress` for each property it queues; with
  nothing cut the array is empty, so that is a bounds check that fails.
  Setup adds one sort check per sequence (the sort already existed) and an
  early return from `cut_interrupted`. Measured below.
- **Scenes that interrupt:** one `f32` per clip in that track, one multiply
  per cut clip sampled, and one `Box` per `.interrupt()` call while building.

Measured on this machine against baselines saved before the change
(`before-interrupt`; noise reaches ~30%):

| Benchmark | Change (Criterion, 95% interval midpoint) |
|---|---|
| core `scrub` 100 / 1k / 10k | −1.9% / −2.9% / −13.2% |
| core `mixed_scrub` 100 / 1k / 10k | −5.4% / −7.5% / −12.7% |
| `setup/animate` 1k / 10k | −6.1% / −3.0% |
| `playback/sample_forward` 1k / 10k | −0.6% (not significant) / −6.4% |
| `playback/seek_backward` 1k / 10k | +2.3% / −8.5% |

None of the benchmarks interrupt, and nothing in the change makes the
uninterrupted path do less work, so the improvements are run-to-run noise;
the one regression (+2.3%) is well inside the same noise. Conclusion: no
measurable cost for scenes that don't interrupt.

## Phase 2: aliasing paths

Decided semantics: `position.x` interrupting `position` cuts only the `x`
component. `position`'s `y` (and any other part) keeps following its curve to
its end. The reverse, `position` interrupting `position.x`, cuts `x` and the
new animation takes over the whole `position`.

A wider interrupter covering a narrower clip (`position` cutting
`position.x`) is a plain cut: the narrower clip is shortened exactly as in
phase 1, and baking already hands its value at the cut to the wider clip
through the shared working copy.

A narrower interrupter (`position.x` cutting `position`) cannot shorten the
wider clip, because its other components keep running. It needs:

- **Write order.** While both run, the narrower clip must write after the
  wider one, and keep winning for `x` after either ends. Sampling orders
  aliasing writes by keyframe distance from the playhead; treating the wider
  clip's keyframe as at most the cut time gives the interrupter priority from
  the cut on, without per-frame cost beyond the existing sort.
- **Start value.** Baking runs clips in start order over one working copy, so
  the wider clip has already written its end before the interrupter is baked.
  The interrupter must start from the wider clip's value at the cut instead:
  baking the wider clip records a copy of the source with that value, and the
  interrupter reads its start from the copy (one source copy per such cut, at
  setup only).
- **Carry forward.** After both, the working copy holds the wider clip's end
  for the other components and the interrupter's end for `x`, which is what
  later clips should start from; baking in start order already gives that.

## Phase 3: springs that keep their velocity

Phase 1 cuts springs by value: smooth in position, with a jump in speed.
Keeping velocity needs:

- **Initial velocity in the response.** `SpringCurve` starts at rest. The
  closed forms extend to an initial velocity, which changes the settle
  duration, so the curve is prepared per action while baking (it is already
  stored per action).
- **Scalar properties only.** A spring drives one 0-to-1 progress value
  between `start` and `end`. For a `Point` or `Color` the old velocity is
  usually not along the new start-to-end line, so one scalar cannot carry it.
  Exact for `f64`/`f32` properties such as `position_x`; multi-component types
  would need per-component springs, as react-spring does.
- **Zero distance.** Normalised velocity is `velocity / (end - start)`; when
  the target equals the current value there is no scale, so the spring starts
  at rest.
- **A cut tween interrupted by a spring:** its velocity is the derivative of
  `interp(start, end, ease(t))`, taken numerically for scalar types.

## Alternatives considered

- **Additive blending** (Core Animation, SwiftUI): smooth for every curve, but
  several clips per property run at once, sampling must sum them, value types
  need addition and subtraction (text and caps have none), and the
  one-clip-per-property binary search no longer holds.
- **Default on** and **a scene-wide switch**: hide real mistakes elsewhere in
  the scene; see "Why opt-in".
- **Cutting in authoring:** authoring sees animation trees, not resolved clip
  times; core already sorts and resolves each property's clips.
- **Storing cuts in the action table** (an optional column, like springs):
  sampling would look the column up every frame, even in scenes that never
  cut. The per-clip array is read where the clip index is already known.

## Tests

Core, `crates/motiongfx/tests/interrupt.rs`: the cut clip keeps its curve and
the interrupter continues from it (including backward seeks); chained
interruptions; an instant interrupter; a cut spring; same-instant and
aliasing clips are not cut; an interrupter overlapping nothing; reserved
duration unchanged.

Authoring, `crates/motiongfx_authoring/tests/animation.rs`: a single change
and a whole group take over (forward and backward); same-instant
interrupters still report `Overlap`.
