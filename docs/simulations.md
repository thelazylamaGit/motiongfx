# Simulations

Status: tier 1 implemented 2026-11-16 (`scene.simulate`,
`scene.simulate_with`); tiers 2 and 3 below are recorded options, not
commitments.

## What a simulation is

A value that advances in fixed steps from its previous state, such as a
falling ball's height and speed, or an angle turned by a rate. It is a
reactive source like `signal(..)` and `computed!`: objects follow it through
their builders or `.map(..)`. Only the simulation's own state is stored;
objects derive from it through ordinary bindings.

```rust
let rate = signal(0.0);
let angle = scene.simulate_with(0.0, &rate, |angle, rate, dt| *angle += rate * dt);
scene.add(Line::new(PIVOT, angle.map(|a| Point::new(-80.0 * a.cos(), -80.0 * a.sin()))));
```

- `simulate(initial, |state, dt| ..)` reads nothing from the scene.
- `simulate_with(initial, inputs, |state, inputs, dt| ..)` reads `inputs` (a
  signal, computed value, property, another simulation, or a tuple of up to
  five) as they are at each step's time. For more than three inputs, combine
  them into a named struct with `computed!` and pass that.
- `dt` is the fixed step length in seconds: `1 / scene.simulation_rate(..)`,
  60 steps per second unless set. Nothing else in MotionGfx steps; tweens,
  springs and signals are sampled at exact times.

Use one when the next state depends on the current one in a way that's
awkward to write as a function of time: collisions, forces, feedback. A
piecewise-linear angle (the rotation example) is equally well written as a
signal animated with linear easing; it is kept as a test because its closed
form checks the stepping exactly.

## Tier 1: run at compile, store every step (implemented)

`scene.compile` builds the timeline, then runs every simulation from time 0
to the scene's end. Step `k` samples the scene at that step's start (so
inputs and other simulations read their values there), calls every
simulation's closure once and stores the result. Sampling at time `t` then
returns the state stored for the last step started at or before `t`, or the
last state past the end. Seeking is exact in both directions and needs no
host support.

- The scene's length is the simulation's length. A simulation-only scene sets
  it with `wait`, for example `scene.compile([wait(s(10))])`.
- The rate is an integer number of steps per second so that each step's start
  time maps back to the same step exactly; 1/60 s is not a whole number of
  nanoseconds, which a `Duration` step would round.
- A frame shows the latest step, so keep the rate at or above the frame rate
  you play or export at, or frames repeat states.
- Each history is a `Vec<T>` reserved to its exact length before running: one
  `T` per step plus the initial state, and nothing else (no inputs, no
  copies of objects). Keep `T` to what the simulation needs.
- Every step samples the whole scene to read inputs, so the cost grows with
  the number of bindings. The step closures are dropped after running.

Measured 2026-11-16 (release, best of five, Windows), compile time including
the run, for the `simulations` example's two simulations:

| Scene | Steps | Compile |
| --- | --- | --- |
| 6 s | 360 | 15 µs |
| 5 min | 18,000 | 0.64 ms |
| 5 min, plus 50 circles following the ball | 18,000 | 9.5 ms |

Implementation: [simulation.rs](../crates/motiongfx_authoring/src/simulation.rs)
(source, storage and stepping) and `CompiledScene::run_simulations` in
[scene.rs](../crates/motiongfx_authoring/src/scene.rs).

## Tier 2: checkpoints and re-simulation (when memory matters)

Store the full state every N steps and each step's inputs; to show time `t`,
restore the nearest earlier checkpoint and step forward with the recorded
inputs. Memory falls from every step to every Nth; a seek costs at most N
steps. This is how game replays, rollback netcode and Unity's timeline
scrubbing work. A 1,000-particle state (16 KB) over 5 minutes is about 288 MB
stored every step and about 5 MB with a checkpoint each second.

It requires determinism: the same code and inputs must give bit-identical
results, because re-simulation must reproduce the original run. Add it as an
option on the same API (for example `.checkpoint_every(s(1))`) when a
simulation's memory becomes a problem; storing every step is a checkpoint
every step.

## Tier 3: cache while playing (live or very expensive simulations)

Blender's simulation zones: step as playback reaches each frame and store as
you go; scrubbing is exact inside what has been played and holds beyond it;
export plays from the start. This deliberately gives up arbitrary-time
sampling, so it would be an explicit, documented exception. Consider it only
for simulations too expensive to run at compile or driven by live input.

## Determinism

Tier 1 doesn't need it: the single run is the recording. Tier 2 does.
Chaotic physics (colliding balls) is still deterministic given identical
computation. What breaks it:

| Cause | Fix |
| --- | --- |
| `dt` from elapsed real time | The fixed step (already enforced) |
| Unseeded randomness | A seeded generator |
| Iterating `HashMap`/`HashSet` (randomly seeded per process) | `Vec`, `BTreeMap`, or a fixed-seed hasher |
| Threads combining results in finishing order | Single-threaded, or combine in a fixed order |
| Live input (mouse, network, clock) | Record it, or tier 3 |
| Different machines or compilers | Fine for previews; bake once for identical output |

## Limits

- A simulation's state type must be `Clone + PartialEq` (it is a source
  value) and is cloned once per step while running.
- Like object handles, a `Simulated<T>` belongs to the scene that created it.
  Using it with another scene is unsupported: it panics if that scene has no
  simulation of type `T` at the same position, and otherwise reads that one.
