# Analytical springs

Status: initial implementation, 2026-11-13. This is a finite analytical animation
primitive, not a frame-stepped simulation or live UI spring controller.

## Ownership

- `motiongfx_interp::spring`: mode-specific settings, checked analytical response,
  position/velocity sampling and finite arrival/settling times. Supports `no_std`.
- `motiongfx`: native spring actions, existing typed target baking/interpolation,
  sampling, continuation and overlap rejection. No graphics/authoring dependency.
- `motiongfx_authoring`: `.to(...).spring(...)` changes for properties and standalone signals.
- `peniko_motiongfx`: opt-in continuous point/vector/size interpolation.
- 2D/Vello/host: normal sampled properties, geometry and rendering. No spring engine.

## Configuration

```rust
let physical = Spring::physical()
    .stiffness(100.0).damping(10.0).mass(1.0);
let visual = Spring::visual(ms(500)).bounce(0.25);
let timed = Spring::timed(ms(500)).bounce(0.4);
```

Physical, visual and timed settings are different types. Physical has no
bounce/timing setter; visual/timed have no stiffness/damping/mass setter.
No `.build()` is required.
Descriptions are validated when a spring is scheduled/prepared. Consumers such
as a curve visualizer can explicitly call `.prepare()` to obtain a reusable,
Copy `SpringCurve` with `.sample(elapsed)`, `.arrival()` and `.duration()`.

| Option | Default | Constraint |
| --- | --- | --- |
| stiffness | 100 | finite, > 0 |
| damping | 10 | finite, > 0 |
| mass | 1 | finite, > 0 |
| visual duration | explicit constructor argument | > 0 |
| timed duration | explicit constructor argument | > 0, <= 1 hour |
| bounce | 0.25 | finite, -1 <= bounce <= 1 |
| rest_delta | 0.001 | finite, > 0 |
| rest_speed | 0.01 | finite, > 0 |

Visual mode follows [Motion's visual-duration parameter mapping](https://github.com/motiondivision/motion/blob/main/packages/motion-dom/src/animation/generators/spring.ts):
nonnegative bounce maps to damping ratio `max(1 - bounce, 0.05)`, and negative
bounce to `1 / max(1 + bounce, 0.05)`. Both endpoints (-1 and 1) are valid;
1 means maximum supported bounce, with finite damping, not endless oscillation.
Negative is overdamped, zero critically
damped, and positive underdamped. The limits keep extreme settings finite.
Physical parameters are unaffected by those limits. All regimes use closed-form
responses. Ratios within 1e-6 of critical use the critical limit for numerical
stability. The response starts at position 0 with velocity 0 and targets 1.

**Rest tolerances are normalized**, unlike Motion's absolute property-unit
thresholds. Delta is a fraction of the start-to-target displacement; speed is
that fraction per second. For a 100-unit transition, delta 0.001 corresponds to
0.1 property units. This deliberate first-version contract keeps timing
independent of target distance: existing track ordering schedules clips before
core target baking. Absolute-unit settling would need distance-aware duration
resolution and is not implemented.

Visual duration controls overall pace, rather than a first crossing or exact end.
The base natural frequency is `TAU / (visual_duration_seconds * 1.2)`. For negative
bounce, frequency is increased so that the position at the requested visual time
matches the critical spring (approximately 96.68%): overdamping changes the shape
without making the approach arbitrarily slower. This scalar calibration happens
once during preparation. Bisection solves the compensation robustly.
Higher positive bounce still produces larger/longer oscillations; visual duration
does not promise a fixed error bound at that time or equal settling times.

`arrival()` remains a diagnostic for the first 95% crossing. It is distinct from
visual duration and does not control sequencing. A conservative decreasing
oscillator-energy bound guarantees BOTH rest tolerances remain satisfied after
the settling time; this can be later than a transient low-speed/low-error sample.
At the finite end, the small residual snaps to the exact target and zero velocity.
The visual trajectory matches Motion's zero-initial-velocity calibration;
settling uses this library's normalized conservative criteria, not Motion's
property-unit rest thresholds, so final durations can differ.

### Exact-duration springs

`Spring::timed(duration).bounce(...)` fits the entire response into the requested
duration, including its settling tail. It uses the same bounce mapping and rest
options as visual springs. Preparation solves for natural frequency using
decreasing analytical bounds on displacement error and speed; both tolerances
are satisfied by the end. Near-critical bounds stay continuous, avoiding unstable
amplitude divisions. No integration or per-sample root finding is involved.

The clip lasts exactly the requested duration and snaps the small residual to
the exact target there. Increasing bounce or tightening tolerances speeds up
the response to fit that time; high bounce and short durations can move rapidly.
Loose displacement tolerances still require at least 95% completion before the
end. This is a conservative calibrated spring, not an earliest-instantaneous-rest
solver or a reproduction of Motion's separate duration solver. `arrival()` remains
the first-95% diagnostic, not the total duration.

Timing must fit between nanosecond resolution and one hour. Non-finite, invalid,
non-settling, or numerically extreme configurations return `SpringError` instead
of entering playback. There is no initial velocity option in this first version.
Prepared coefficients are private and validated.

## Authoring and scheduling

```rust
scene.compile([
    ball.position_x().to(200.0).spring(physical),
    ball.position_x().to(-200.0).spring(visual),
    progress.to(1.0).spring(physical),
    ball.position_x().to(200.0).spring(timed),
]);
```

A spring cannot be combined with `.over()` or `.ease()` (that panics at the
call). Springs lower to the same
core primitive available through `builder.spring(id, path, target_fn, settings)`
or `builder.act_builder(...).with_spring(settings)`. Core builders expose
`.play()` and checked `.try_play()` without an explicit duration argument.

Physical/visual springs reserve their calculated settling duration; timed
springs reserve exactly their requested duration. Ordinary sequential actions
start after that clip ends; parallel, staggered and other composition use the existing
core ordering combinators without changes. There is no arrival-based continuation.

Existing conflict rules apply to all actions, including springs. A later-authored
overlapping write on the same or an aliasing property can drop the earlier clip
entirely; it does not preserve a prefix or inherit velocity. This initial spring
implementation neither changes that policy nor adds special overlap rejection.
General interruption/live retargeting is deferred to future timeline work.

`SpringValue<M>` opts continuous types into overshoot-capable interpolation
for core's `ActionBuilder::with_spring`. The core supports f32/f64; Peniko
integration supplies Point/Vec2/Size. Custom integrations can opt in with their
existing interpolation marker type.

`InterpActionBuilder::with_spring` instead springs whatever interpolation the
action already has, with no `SpringValue` bound. Authoring uses it, so any
interpolated property can spring; only discrete properties are refused.
Overshoot is the interpolation's own extrapolation: colour channels briefly
leave 0–1 and rendering saturates them (an alpha overshoot brightens slightly,
since colours are premultiplied first). Tween colours, or spring them in a
colour space suited to overshoot.

## Errors and compatibility

`SpringError` covers invalid spring settings. Core track compilation is unchanged.
`Scene::try_compile` returns `SceneError`:

- `SceneError::Binding(BindingError)` for existing relationship diagnostics.
- `SceneError::Animation(Vec<AnimationError>)` for invalid spring settings
  (`AnimationErrorKind::Spring(SpringError)`) and springs on discrete
  properties, each with the animation's file and line.

Code matching the old direct `BindingError` return needs to match its wrapper.
`Scene::compile` is the panicking convenience. Core low-level target baking,
queueing and sampling calls are otherwise unchanged.

## Demo and future scope

Run `cargo run -p vello_winit_example --example spring_curves`.
It compares physical, visual and timed responses, using actual `.to(...).spring(...)`
actions for three moving balls and synchronized response markers. Edit/add
presets in `experiments()` using the ordinary Spring builders; every setting is
available, with no separate demo-specific parameter API. Settings smoothly
interpolate between settled runs through signals. Balls alternate direction
automatically. Plots share a continuously fitted time axis and display current
stiffness/damping/mass or duration/bounce alongside settling-time readouts.
The graph uses the same public sampler as core spring actions.

`Spring` descriptions implement bounded `Interpolation<()>`: same-mode settings
interpolate all parameters, including duration and rest tolerances; different
modes switch at the end. `Display` formats the main parameters for diagnostics.
This supports `signal(settings).to(next_settings)` in visualizers, not changing
the coefficients of an already prepared spring action. Invalid inputs still
require preparation validation.

Curves are sampled into ordinary Path geometry only when parameters change;
Vello retains its normal path cache. Native per-sample spring calculations still
have a cost; this feature is not a zero-cost easing function or a GPU simulation.

The implementation does not change active spring settings, inherit velocity,
blend overlapping actions, or handle live retargeting. Future interruption must
preserve the previous clip before takeover and seed the replacement from sampled
position/velocity, retaining the original time mapping when truncating a clip.
Interactive events additionally need a history policy for reproducible seeking.
