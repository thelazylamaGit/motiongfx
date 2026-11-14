use core::time::Duration;
use motiongfx_interp::spring::{Spring, SpringCurve};
fn time(seconds: f64) -> Duration {
    Duration::from_secs_f64(seconds)
}

#[test]
fn all_damping_regimes_satisfy_the_oscillator_equation_and_velocity()
{
    for damping in [6.0, 20.0, 32.0, 19.99999, 20.00001] {
        let curve = Spring::physical()
            .stiffness(100.0)
            .damping(damping)
            .prepare()
            .unwrap();
        for t in [0.02, 0.1, 0.3] {
            let h = 0.0001;
            let a = curve.sample(time(t - h));
            let b = curve.sample(time(t));
            let c = curve.sample(time(t + h));
            let derivative = (c.position - a.position) / (2.0 * h);
            let acceleration = (c.position - 2.0 * b.position
                + a.position)
                / (h * h);
            assert!((derivative - b.velocity).abs() < 0.0001);
            assert!(
                (acceleration
                    + damping * b.velocity
                    + 100.0 * (b.position - 1.0))
                    .abs()
                    < 0.001
            );
        }
    }
}

#[test]
fn visual_duration_scales_the_response_including_overdamping() {
    for bounce in [-1.0, -0.5, 0.0, 0.25, 0.8] {
        for duration in [time(0.05), time(0.5), time(2.0)] {
            let curve = Spring::visual(duration)
                .bounce(bounce)
                .prepare()
                .unwrap();
            let reference = Spring::visual(time(0.5))
                .bounce(bounce)
                .prepare()
                .unwrap();
            let scale = duration.as_secs_f64() / 0.5;
            for fraction in [0.2, 0.5, 1.0] {
                let actual = curve.sample(duration.mul_f64(fraction));
                let expected = reference.sample(time(0.5 * fraction));
                assert!(
                    (actual.position - expected.position).abs()
                        < 1e-9
                );
                assert!(
                    (actual.velocity * scale - expected.velocity)
                        .abs()
                        < 1e-8
                );
            }
            assert!(
                (curve.sample(curve.arrival()).position - 0.95).abs()
                    < 1e-6
            );
            assert!(curve.duration() > curve.arrival());
            let peak = (0..=1000)
                .map(|i| {
                    curve
                        .sample(
                            curve
                                .duration()
                                .mul_f64(i as f64 / 1000.0),
                        )
                        .position
                })
                .fold(0.0, f64::max);
            assert_eq!(peak > 1.00001, bounce > 0.0);
        }
    }
}

#[test]
fn settlement_is_finite_exact_repeatable_and_sensitive_to_tolerances()
{
    let loose = Spring::physical().prepare().unwrap();
    let tight = Spring::physical()
        .rest_delta(0.00001)
        .rest_speed(0.0001)
        .prepare()
        .unwrap();
    assert!(tight.duration() > loose.duration());
    for curve in [loose, tight] {
        let start = curve.sample(Duration::ZERO);
        assert_eq!(start.position, 0.0);
        assert_eq!(start.velocity, 0.0);
        let a = curve.sample(time(0.2));
        curve.sample(time(0.7));
        assert_eq!(a, curve.sample(time(0.2)));
        let before =
            curve.sample(curve.duration() - Duration::from_nanos(2));
        assert!((1.0 - before.position).abs() < 0.0011);
        assert!(before.velocity.abs() < 0.0101);
        assert_eq!(curve.sample(curve.duration()).position, 1.0);
        assert_eq!(curve.sample(Duration::MAX).velocity, 0.0);
    }
}

#[test]
fn invalid_and_non_settling_settings_are_errors_not_non_finite_curves()
 {
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            Spring::physical().stiffness(value).prepare().is_err()
        );
        assert!(Spring::physical().mass(value).prepare().is_err());
        assert!(Spring::physical().damping(value).prepare().is_err());
        assert!(
            Spring::physical().rest_delta(value).prepare().is_err()
        );
        assert!(
            Spring::physical().rest_speed(value).prepare().is_err()
        );
    }
    for value in [-1.1, 1.0001, 2.0, f64::NAN] {
        assert!(
            Spring::visual(time(0.5))
                .bounce(value)
                .prepare()
                .is_err()
        );
    }
    assert!(Spring::visual(Duration::ZERO).prepare().is_err());
    assert!(Spring::physical().damping(1e-100).prepare().is_err());
    assert!(Spring::physical().stiffness(1e300).prepare().is_err());
}

// Keep this type annotation: prepared samplers are Copy and renderer-independent.
const _: fn(SpringCurve) -> SpringCurve = |curve| {
    let _copy = curve;
    curve
};

#[test]
fn visual_response_matches_motion_reference_values() {
    // Zero-velocity samples from Motion's visualDuration=0.5 calibration,
    // including its overdamped frequency compensation and high-bounce limit.
    // https://github.com/motiondivision/motion/blob/main/packages/motion-dom/src/animation/generators/spring.ts
    // Compare the analytical trajectory, not Motion's different rest cutoff.
    for (bounce, seconds, position, velocity) in [
        (-1.0, 0.1, 0.4937017573412813, 3.449183861611964),
        (-1.0, 0.5, 0.9668147828315378, 0.22607606714270906),
        (-0.6, 0.1, 0.47559959281649056, 3.6185598982995058),
        (-0.6, 0.5, 0.9668147828315379, 0.22899052829383243),
        (0.0, 0.1, 0.2815978300779749, 3.848266303730527),
        (0.0, 0.5, 0.9668147828315378, 0.29178747816808576),
        (0.25, 0.5, 1.025755677380834, -0.09862505641110998),
        (0.7, 0.5, 1.0048520999202661, -2.1916134680143857),
        (0.7, 0.75, 0.9385774941442888, 0.9731169681677445),
        (0.98, 0.5, 0.6530354875339733, -7.01511020012497),
    ] {
        let curve = Spring::visual(time(0.5))
            .bounce(bounce)
            .prepare()
            .unwrap();
        let sample = curve.sample(time(seconds));
        assert!(
            (sample.position - position).abs() < 1e-9,
            "bounce {bounce} at {seconds}s"
        );
        assert!(
            (sample.velocity - velocity).abs() < 1e-8,
            "bounce {bounce} at {seconds}s"
        );
    }
}

#[test]
fn negative_bounce_keeps_visual_position_aligned_with_critical_damping()
 {
    for seconds in [0.05, 0.5, 2.0] {
        let duration = time(seconds);
        let critical =
            Spring::visual(duration).bounce(0.0).prepare().unwrap();
        for bounce in [-1.0, -0.6, -0.01, -1e-8] {
            let curve = Spring::visual(duration)
                .bounce(bounce)
                .prepare()
                .unwrap();
            assert!(
                (curve.sample(duration).position
                    - critical.sample(duration).position)
                    .abs()
                    < 1e-9
            );
        }
    }
}

#[test]
fn timed_springs_fit_rest_tolerances_and_end_at_exact_duration() {
    for seconds in [0.05, 0.5, 2.0] {
        for bounce in [-1.0, -0.6, -1e-8, 0.0, 1e-8, 0.4, 0.9, 0.99] {
            let duration = time(seconds);
            let curve = Spring::timed(duration)
                .bounce(bounce)
                .prepare()
                .unwrap();
            assert_eq!(curve.duration(), duration);
            assert!(curve.arrival() < duration);
            assert_eq!(curve.sample(Duration::ZERO).position, 0.0);
            assert_eq!(curve.sample(Duration::ZERO).velocity, 0.0);
            // Inspect the unsnapped response just before the end.
            let before =
                curve.sample(duration - Duration::from_nanos(1));
            assert!((1.0 - before.position).abs() <= 0.001001);
            assert!(before.velocity.abs() <= 0.01001);
            assert_eq!(curve.sample(duration).position, 1.0);
            assert_eq!(curve.sample(duration).velocity, 0.0);
            assert_eq!(curve.sample(Duration::MAX).position, 1.0);
            let middle = curve.sample(duration / 2);
            curve.sample(duration);
            assert_eq!(curve.sample(duration / 2), middle);
            let peak = (0..=2000)
                .map(|i| {
                    curve
                        .sample(duration.mul_f64(i as f64 / 2000.0))
                        .position
                })
                .fold(0.0, f64::max);
            // Very small positive bounce can have imperceptibly small overshoot.
            if bounce >= 0.4 {
                assert!(peak > 1.01);
            }
            if bounce <= 0.0 {
                assert!(peak <= 1.0);
            }
        }
    }
}

#[test]
fn timed_rest_overrides_change_the_response_without_changing_duration()
 {
    let duration = time(0.5);
    for bounce in [-0.6, 0.0, 0.7] {
        let loose =
            Spring::timed(duration).bounce(bounce).prepare().unwrap();
        let tight = Spring::timed(duration)
            .bounce(bounce)
            .rest_delta(1e-6)
            .rest_speed(1e-5)
            .prepare()
            .unwrap();
        assert_eq!(tight.duration(), loose.duration());
        let before = tight.sample(duration - Duration::from_nanos(1));
        assert!((1.0 - before.position).abs() <= 1.001e-6);
        assert!(before.velocity.abs() <= 1.001e-5);
        assert_ne!(
            tight.sample(duration / 8),
            loose.sample(duration / 8)
        );
    }
    assert!(Spring::timed(Duration::ZERO).prepare().is_err());
    assert!(Spring::timed(time(3601.0)).prepare().is_err());
    for value in [-1.1, 1.0001, f64::NAN, f64::INFINITY] {
        assert!(
            Spring::timed(duration).bounce(value).prepare().is_err()
        );
    }
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            Spring::timed(duration)
                .rest_delta(value)
                .prepare()
                .is_err()
        );
        assert!(
            Spring::timed(duration)
                .rest_speed(value)
                .prepare()
                .is_err()
        );
    }
}

#[test]
fn settings_interpolation_covers_all_parameters_and_preserves_modes()
{
    use motiongfx_interp::interpolation::Interpolation;
    let cases: [(Spring, Spring, Spring); 3] = [
        (
            Spring::physical()
                .stiffness(100.0)
                .damping(10.0)
                .mass(1.0)
                .rest_delta(0.001)
                .rest_speed(0.01)
                .into(),
            Spring::physical()
                .stiffness(300.0)
                .damping(20.0)
                .mass(2.0)
                .rest_delta(0.002)
                .rest_speed(0.02)
                .into(),
            Spring::physical()
                .stiffness(200.0)
                .damping(15.0)
                .mass(1.5)
                .rest_delta(0.0015)
                .rest_speed(0.015)
                .into(),
        ),
        (
            Spring::visual(time(0.5))
                .bounce(0.2)
                .rest_delta(0.001)
                .rest_speed(0.01)
                .into(),
            Spring::visual(time(1.0))
                .bounce(0.6)
                .rest_delta(0.002)
                .rest_speed(0.02)
                .into(),
            Spring::visual(time(0.75))
                .bounce(0.4)
                .rest_delta(0.0015)
                .rest_speed(0.015)
                .into(),
        ),
        (
            Spring::timed(time(0.5))
                .bounce(0.2)
                .rest_delta(0.001)
                .rest_speed(0.01)
                .into(),
            Spring::timed(time(1.0))
                .bounce(0.6)
                .rest_delta(0.002)
                .rest_speed(0.02)
                .into(),
            Spring::timed(time(0.75))
                .bounce(0.4)
                .rest_delta(0.0015)
                .rest_speed(0.015)
                .into(),
        ),
    ];
    for (a, b, expected) in cases {
        let middle = Spring::interp(&a, &b, 0.5).prepare().unwrap();
        let expected = expected.prepare().unwrap();
        assert!(
            middle.duration().abs_diff(expected.duration())
                < Duration::from_nanos(2)
        );
        for seconds in [0.02, 0.1, 0.2] {
            assert!(
                (middle.sample(time(seconds)).position
                    - expected.sample(time(seconds)).position)
                    .abs()
                    < 1e-9
            );
        }
        assert_eq!(Spring::interp(&a, &b, -0.2), a);
        assert_eq!(Spring::interp(&a, &b, 1.2), b);
        assert_eq!(Spring::interp(&a, &b, f32::NAN), a);
    }
    let physical: Spring = Spring::physical().into();
    let timed: Spring = Spring::timed(time(0.5)).into();
    assert_eq!(Spring::interp(&physical, &timed, 0.5), physical);
    assert_eq!(Spring::interp(&physical, &timed, 1.0), timed);
}

#[test]
fn maximum_bounce_is_valid_and_uses_the_finite_damping_limit() {
    for (maximum, limited) in [
        (
            Spring::visual(time(0.65)).bounce(1.0).prepare().unwrap(),
            Spring::visual(time(0.65))
                .bounce(0.99)
                .prepare()
                .unwrap(),
        ),
        (
            Spring::timed(time(0.65)).bounce(1.0).prepare().unwrap(),
            Spring::timed(time(0.65)).bounce(0.99).prepare().unwrap(),
        ),
    ] {
        assert_eq!(maximum, limited);
        assert!(maximum.duration() > Duration::ZERO);
        assert_eq!(maximum.sample(maximum.duration()).position, 1.0);
        assert_eq!(maximum.sample(maximum.duration()).velocity, 0.0);
    }
}

#[test]
fn duration_accessors_match_the_scheduled_curve() {
    let timed = Spring::timed(time(0.65)).bounce(0.65);
    assert_eq!(timed.duration(), time(0.65));
    assert_eq!(timed.settle_duration().unwrap(), timed.duration());

    let visual = Spring::visual(time(0.4)).bounce(0.5);
    assert_eq!(visual.visual_duration(), time(0.4));
    let settle = visual.settle_duration().unwrap();
    assert_eq!(settle, visual.prepare().unwrap().duration());
    assert!(settle > visual.visual_duration());

    let physical = Spring::physical();
    assert_eq!(
        Spring::from(physical).settle_duration().unwrap(),
        physical.prepare().unwrap().duration()
    );
    assert!(Spring::physical().mass(0.0).settle_duration().is_err());
}
