//! Analytical, initially resting spring responses from zero to one.
//!
//! Settings are construction descriptions; `prepare` validates once and creates
//! a copyable sampler. All times are seconds internally. Rest tolerances use
//! normalized displacement and normalized displacement/second, so scheduling
//! does not depend on an action's target distance. This is not a live simulator.
use crate::interpolation::Interpolation;
use core::{fmt, time::Duration};

const MAX_SECONDS: f64 = 3600.0;
const ARRIVAL: f64 = 0.95;

/// Mutually exclusive ways to configure the same analytical response.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spring {
    Physical(PhysicalSpring),
    Visual(VisualSpring),
    Timed(TimedSpring),
}

/// Physics controls. Numerical values are checked during preparation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalSpring {
    stiffness: f64,
    damping: f64,
    mass: f64,
    rest: Rest,
}

/// Visual timing and damping controls, without physical parameter setters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualSpring {
    duration: Duration,
    bounce: f64,
    rest: Rest,
}

/// Exact total duration and bounce, without physical parameter setters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedSpring {
    duration: Duration,
    bounce: f64,
    rest: Rest,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rest {
    delta: f64,
    speed: f64,
}
impl Default for Rest {
    fn default() -> Self {
        Self {
            delta: 0.001,
            speed: 0.01,
        }
    }
}
impl Default for PhysicalSpring {
    fn default() -> Self {
        Self {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
            rest: Rest::default(),
        }
    }
}
impl Spring {
    pub fn physical() -> PhysicalSpring {
        PhysicalSpring::default()
    }
    /// Sets the overall pace using Motion's visual-duration calibration.
    /// Bounce defaults to 0.25. The settling tail can outlast this duration;
    /// this is neither a fixed end time nor a first-target-crossing time.
    pub fn visual(duration: Duration) -> VisualSpring {
        VisualSpring {
            duration,
            bounce: 0.25,
            rest: Rest::default(),
        }
    }
    /// Fits the motion and settling tail into exactly `duration`.
    /// Higher bounce requires faster motion to settle within the same time.
    /// Bounce defaults to 0.25; rest tolerances control the final residual.
    pub fn timed(duration: Duration) -> TimedSpring {
        TimedSpring {
            duration,
            bounce: 0.25,
            rest: Rest::default(),
        }
    }
    /// Total scheduled time until the spring rests: the time a spring change
    /// reserves on the timeline. Timed springs return their exact duration.
    /// Fails for the same invalid settings as [`Spring::prepare`].
    pub fn settle_duration(self) -> Result<Duration, SpringError> {
        self.prepare().map(SpringCurve::duration)
    }
    pub fn prepare(self) -> Result<SpringCurve, SpringError> {
        let (frequency, ratio, rest) = match self {
            Self::Physical(s) => {
                positive(s.stiffness, "stiffness")?;
                positive(s.damping, "damping")?;
                positive(s.mass, "mass")?;
                let frequency = sqrt(s.stiffness / s.mass);
                (
                    frequency,
                    s.damping / (2.0 * s.mass * frequency),
                    s.rest,
                )
            }
            Self::Visual(s) => {
                let seconds = s.duration.as_secs_f64();
                positive(seconds, "visual duration")?;
                // Match Motion's visual-duration parameter mapping. Limit both
                // extremes so high bounce still decays and negative bounce
                // still moves. Physical settings are unaffected by this limit.
                let ratio = bounce_ratio(s.bounce)?;
                let phase = core::f64::consts::TAU / 1.2;
                let frequency = if ratio > 1.0 {
                    // Calibrate an overdamped response to the critical spring's
                    // position at the requested visual time. Solve in unit
                    // frequency/time, then scale to seconds. Bisection avoids
                    // Newton convergence issues near critical damping.
                    let target = Response::new(1.0, 1.0)?
                        .sample(phase)
                        .position;
                    let unit = Response::new(1.0, ratio)?;
                    first_time(
                        |t| unit.sample(t).position >= target,
                        phase,
                    )? / seconds
                } else {
                    phase / seconds
                };
                (frequency, ratio, s.rest)
            }
            Self::Timed(s) => return timed_curve(s),
        };
        positive(rest.delta, "rest delta")?;
        positive(rest.speed, "rest speed")?;
        let response = Response::new(frequency, ratio)?;
        let arrival = arrival(response)?;
        // Energy decreases monotonically. This bound guarantees BOTH rest
        // tolerances remain satisfied, rather than stopping at a transient
        // target crossing or an overshoot's turning point.
        let limit = (rest.delta * rest.delta)
            .min((rest.speed / frequency) * (rest.speed / frequency));
        positive(limit, "rest tolerances at this frequency")?;
        let settled = first_time(
            |t| {
                let sample = response.sample(t);
                let error = 1.0 - sample.position;
                error * error
                    + (sample.velocity / frequency)
                        * (sample.velocity / frequency)
                    <= limit
            },
            arrival.max(1.0 / frequency),
        )?
        .max(arrival);
        let duration = Duration::try_from_secs_f64(settled)
            .map_err(|_| SpringError::UnrepresentableTiming)?;
        let arrival = Duration::try_from_secs_f64(arrival)
            .map_err(|_| SpringError::UnrepresentableTiming)?;
        if duration.is_zero() || arrival.is_zero() {
            return Err(SpringError::UnrepresentableTiming);
        }
        Ok(SpringCurve {
            response,
            duration,
            arrival,
        })
    }
}

macro_rules! settings {
    ($ty:ty) => {
        impl $ty {
            /// Normalized displacement tolerance, default 0.001 (0.1%).
            pub fn rest_delta(mut self, value: f64) -> Self {
                self.rest.delta = value;
                self
            }
            /// Normalized displacement/second tolerance, default 0.01.
            pub fn rest_speed(mut self, value: f64) -> Self {
                self.rest.speed = value;
                self
            }
            /// See [`Spring::settle_duration`].
            pub fn settle_duration(
                self,
            ) -> Result<Duration, SpringError> {
                Spring::from(self).settle_duration()
            }
            pub fn prepare(self) -> Result<SpringCurve, SpringError> {
                Spring::from(self).prepare()
            }
        }
    };
}
settings!(PhysicalSpring);
settings!(VisualSpring);
settings!(TimedSpring);
impl PhysicalSpring {
    pub fn stiffness(mut self, value: f64) -> Self {
        self.stiffness = value;
        self
    }
    pub fn damping(mut self, value: f64) -> Self {
        self.damping = value;
        self
    }
    pub fn mass(mut self, value: f64) -> Self {
        self.mass = value;
        self
    }
}
impl VisualSpring {
    /// The configured pace. The settling tail usually outlasts it; use
    /// [`VisualSpring::settle_duration`] for the reserved timeline time.
    pub fn visual_duration(self) -> Duration {
        self.duration
    }
    /// Negative values are overdamped, 0 critically damped; positive values
    /// are underdamped. Accepts -1 <= bounce <= 1, limiting the derived damping
    /// ratio to 0.05..=20 at the extremes, as in Motion's visual mapping.
    pub fn bounce(mut self, value: f64) -> Self {
        self.bounce = value;
        self
    }
}
impl TimedSpring {
    /// Exact total time the spring reserves, including its settling tail.
    /// Infallible, so it can time sibling tweens: `.over(spring.duration())`.
    pub fn duration(self) -> Duration {
        self.duration
    }
    /// Uses the same -1 <= bounce <= 1 damping mapping as visual springs.
    pub fn bounce(mut self, value: f64) -> Self {
        self.bounce = value;
        self
    }
}
impl From<PhysicalSpring> for Spring {
    fn from(s: PhysicalSpring) -> Self {
        Self::Physical(s)
    }
}
impl From<VisualSpring> for Spring {
    fn from(s: VisualSpring) -> Self {
        Self::Visual(s)
    }
}
impl From<TimedSpring> for Spring {
    fn from(s: TimedSpring) -> Self {
        Self::Timed(s)
    }
}

/// Interpolates settings descriptions for visualizers, not an active spring.
/// Progress is bounded to preserve valid parameter ranges between valid inputs.
/// Different configuration modes switch at the end rather than mixing units.
impl Interpolation<()> for Spring {
    fn interp(a: &Self, b: &Self, t: f32) -> Self {
        if t.is_nan() || t <= 0.0 {
            return *a;
        }
        if t >= 1.0 {
            return *b;
        }
        let t = f64::from(t);
        let mix = |a: f64, b: f64| a + (b - a) * t;
        let rest = |a: Rest, b: Rest| Rest {
            delta: mix(a.delta, b.delta),
            speed: mix(a.speed, b.speed),
        };
        let duration = |a: Duration, b: Duration| {
            // Interpolate the difference, retaining precision near large times.
            if a <= b {
                a + (b - a).mul_f64(t)
            } else {
                a - (a - b).mul_f64(t)
            }
        };
        match (*a, *b) {
            (Self::Physical(a), Self::Physical(b)) => {
                Self::Physical(PhysicalSpring {
                    stiffness: mix(a.stiffness, b.stiffness),
                    damping: mix(a.damping, b.damping),
                    mass: mix(a.mass, b.mass),
                    rest: rest(a.rest, b.rest),
                })
            }
            (Self::Visual(a), Self::Visual(b)) => {
                Self::Visual(VisualSpring {
                    duration: duration(a.duration, b.duration),
                    bounce: mix(a.bounce, b.bounce),
                    rest: rest(a.rest, b.rest),
                })
            }
            (Self::Timed(a), Self::Timed(b)) => {
                Self::Timed(TimedSpring {
                    duration: duration(a.duration, b.duration),
                    bounce: mix(a.bounce, b.bounce),
                    rest: rest(a.rest, b.rest),
                })
            }
            _ => *a,
        }
    }
}

/// Main authoring parameters, suitable for diagnostic labels and logs.
impl fmt::Display for Spring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Physical(s) => write!(
                f,
                "STIFFNESS {:.1} / DAMPING {:.1} / MASS {:.2}",
                s.stiffness, s.damping, s.mass
            ),
            Self::Visual(s) => write!(
                f,
                "VISUAL DURATION {:.2}S / BOUNCE {:.2}",
                s.duration.as_secs_f64(),
                s.bounce
            ),
            Self::Timed(s) => write!(
                f,
                "DURATION {:.2}S / BOUNCE {:.2}",
                s.duration.as_secs_f64(),
                s.bounce
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpringError {
    InvalidParameter(&'static str),
    /// Timing must be representable at nanosecond resolution and <= one hour.
    UnrepresentableTiming,
}
impl fmt::Display for SpringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameter(name) => write!(
                f,
                "invalid spring {name}: expected a finite positive value unless otherwise specified"
            ),
            Self::UnrepresentableTiming => write!(
                f,
                "spring timing is outside the supported nanosecond-to-one-hour range"
            ),
        }
    }
}
impl core::error::Error for SpringError {}

/// Validated, allocation-free response, reusable by core actions and visualizers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringCurve {
    response: Response,
    duration: Duration,
    arrival: Duration,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringSample {
    pub position: f64,
    /// Normalized displacement per second.
    pub velocity: f64,
}
impl SpringCurve {
    pub fn duration(self) -> Duration {
        self.duration
    }
    /// First reaches 95% of the target. This diagnostic milestone is distinct
    /// from the visual-duration setting and is not used for scheduling.
    pub fn arrival(self) -> Duration {
        self.arrival
    }
    /// Arbitrary absolute elapsed time, including backward samples. At the finite
    /// end, the tiny residual is snapped to the exact target with zero velocity.
    pub fn sample(self, time: Duration) -> SpringSample {
        if time >= self.duration {
            return SpringSample {
                position: 1.0,
                velocity: 0.0,
            };
        }
        self.response.sample(time.as_secs_f64())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Response {
    frequency: f64,
    regime: Regime,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Regime {
    Under { decay: f64, angular: f64 },
    Critical,
    Over { slow: f64, fast: f64 },
}
impl Response {
    fn new(frequency: f64, ratio: f64) -> Result<Self, SpringError> {
        positive(frequency, "derived frequency")?;
        positive(ratio, "derived damping ratio")?;
        let regime = if (ratio - 1.0).abs() < 1e-6 {
            Regime::Critical
        } else if ratio < 1.0 {
            Regime::Under {
                decay: ratio * frequency,
                angular: frequency * sqrt(1.0 - ratio * ratio),
            }
        } else {
            let factor = ratio + sqrt(ratio * ratio - 1.0);
            positive(factor, "derived damping coefficients")?;
            Regime::Over {
                slow: frequency / factor,
                fast: frequency * factor,
            }
        };
        Ok(Self { frequency, regime })
    }
    fn sample(self, t: f64) -> SpringSample {
        if t == 0.0 {
            return SpringSample {
                position: 0.0,
                velocity: 0.0,
            };
        }
        let w = self.frequency;
        let (error, velocity) = match self.regime {
            Regime::Under { decay, angular } => {
                let e = exp(-decay * t);
                let s = sin(angular * t) / angular;
                (e * (cos(angular * t) + decay * s), e * w * w * s)
            }
            Regime::Critical => {
                let e = exp(-w * t);
                (e * (1.0 + w * t), w * w * t * e)
            }
            Regime::Over { slow, fast } => {
                let a = exp(-slow * t);
                let b = exp(-fast * t);
                (
                    (fast * a - slow * b) / (fast - slow),
                    slow * fast * (a - b) / (fast - slow),
                )
            }
        };
        SpringSample {
            position: 1.0 - error,
            velocity,
        }
    }
}

fn bounce_ratio(bounce: f64) -> Result<f64, SpringError> {
    if !bounce.is_finite() || !(-1.0..=1.0).contains(&bounce) {
        return Err(SpringError::InvalidParameter(
            "bounce (expected -1 <= bounce <= 1)",
        ));
    }
    Ok(if bounce < 0.0 {
        1.0 / (1.0 + bounce).max(0.05)
    } else {
        (1.0 - bounce).max(0.05)
    })
}

fn timed_curve(s: TimedSpring) -> Result<SpringCurve, SpringError> {
    let seconds = s.duration.as_secs_f64();
    positive(seconds, "timed duration")?;
    if seconds > MAX_SECONDS {
        return Err(SpringError::UnrepresentableTiming);
    }
    positive(s.rest.delta, "rest delta")?;
    positive(s.rest.speed, "rest speed")?;
    let ratio = bounce_ratio(s.bounce)?;
    let unit = Response::new(1.0, ratio)?;

    // Solve in dimensionless phase = frequency * duration. These analytical
    // envelopes bound error and speed for the entire tail, avoiding a false
    // finish at a target crossing. Start where BOTH bounds decrease with phase.
    // Frequency is calibrated once; playback keeps the ordinary response.
    let start = match unit.regime {
        Regime::Under { decay, .. } => 2.0 / decay,
        Regime::Critical => 2.0,
        Regime::Over { slow, .. } => 2.0 / slow,
    };
    let delta = s.rest.delta.min(1.0 - ARRIVAL);
    let phase = first_match(
        |phase| {
            let (error, speed) = rest_envelope(unit, phase);
            phase >= start
                && error <= delta
                && speed * (phase / seconds) <= s.rest.speed
        },
        start,
        f64::MAX,
    )?;
    let response = Response::new(phase / seconds, ratio)?;
    let arrival = Duration::try_from_secs_f64(arrival(response)?)
        .map_err(|_| SpringError::UnrepresentableTiming)?;
    if arrival.is_zero() {
        return Err(SpringError::UnrepresentableTiming);
    }
    Ok(SpringCurve {
        response,
        duration: s.duration,
        arrival,
    })
}

/// Tail bounds for a unit-frequency response: displacement error and speed.
/// Polynomial bounds stay continuous near critical damping; amplitude bounds
/// tighten them away from critical. No per-frame use of these envelopes.
fn rest_envelope(unit: Response, phase: f64) -> (f64, f64) {
    let (decay, error, speed) = match unit.regime {
        Regime::Under { decay, angular } => (
            decay,
            (1.0 + decay * phase).min(1.0 / angular),
            phase.min(1.0 / angular),
        ),
        Regime::Critical => (1.0, 1.0 + phase, phase),
        Regime::Over { slow, fast } => (
            slow,
            (1.0 + slow * phase).min(fast / (fast - slow)),
            phase.min(1.0 / (fast - slow)),
        ),
    };
    let envelope = exp(-decay * phase);
    (error * envelope, speed * envelope)
}

fn positive(
    value: f64,
    name: &'static str,
) -> Result<(), SpringError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(SpringError::InvalidParameter(name))
    }
}
fn arrival(response: Response) -> Result<f64, SpringError> {
    let upper = match response.regime {
        Regime::Under { angular, .. } => {
            core::f64::consts::PI / angular
        }
        _ => 1.0 / response.frequency,
    };
    first_time(|t| response.sample(t).position >= ARRIVAL, upper)
}
fn first_time(
    test: impl Fn(f64) -> bool,
    upper: f64,
) -> Result<f64, SpringError> {
    first_match(test, upper, MAX_SECONDS)
}

fn first_match(
    test: impl Fn(f64) -> bool,
    upper: f64,
    limit: f64,
) -> Result<f64, SpringError> {
    let mut hi = upper.min(limit);
    if !hi.is_finite() || hi <= 0.0 {
        return Err(SpringError::UnrepresentableTiming);
    }
    while !test(hi) {
        if hi >= limit {
            return Err(SpringError::UnrepresentableTiming);
        }
        hi = (hi * 2.0).min(limit);
    }
    let mut lo = 0.0;
    for _ in 0..64 {
        let mid = (lo + hi) * 0.5;
        if test(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(hi)
}
macro_rules! math {
    ($($name:ident),*) => {
        $(
            fn $name(value: f64) -> f64 {
                #[cfg(feature = "std")]
                { value.$name() }
                #[cfg(not(feature = "std"))]
                { libm::$name(value) }
            }
        )*
    };
}
math!(sqrt, exp, sin, cos);

/// Opt-in for continuous values whose interpolation supports spring overshoot.
/// The marker permits downstream implementations for foreign geometry types.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be animated with a spring",
    label = "`{Self}` does not implement `SpringValue`",
    note = "springs overshoot their target, so only continuous values that stay valid past it (scalars, points, vectors, sizes) opt in",
    note = "use a tween instead, e.g. `.over(duration)` / `.ease(...)`"
)]
pub trait SpringValue<M>:
    crate::interpolation::Interpolation<M>
{
}
impl SpringValue<()> for f32 {}
impl SpringValue<()> for f64 {}
