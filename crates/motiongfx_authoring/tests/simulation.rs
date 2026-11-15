//! Simulations: fixed steps run at compile, stored, and looked up by time.
use motiongfx::prelude::{ms, s};
use motiongfx_authoring::{Scene, computed, signal, wait};
use std::time::Duration;

/// Manim's `RotationUpdater`: the angle is the integral of a rate the
/// timeline switches between 1, -1 and 0 rad/s, so it has a closed form.
#[test]
fn integrating_a_switched_rate_matches_the_closed_form() {
    let rate = signal(0.0);
    let mut scene = Scene::<()>::new();
    let angle = scene.simulate_with(0.0, &rate, |angle, rate, dt| {
        *angle += rate * dt
    });
    let mut animation = scene.compile([
        rate.set(1.0),
        wait(s(2)),
        rate.set(-1.0),
        wait(s(2)),
        rate.set(0.0),
        wait(ms(500)),
    ]);

    for (time, expected) in [
        (ms(0), 0.0),
        (ms(500), 0.5),
        (s(2), 2.0),
        (s(3), 1.0),
        (s(4), 0.0),
        (ms(4500), 0.0),
        // Backward seeks read the same stored steps.
        (s(1), 1.0),
        (ms(2500), 1.5),
    ] {
        animation.sample(time);
        let angle = *animation.simulated(angle);
        assert!(
            (angle - expected).abs() < 1e-9,
            "at {time:?}: {angle} (expected {expected})"
        );
    }
}

#[test]
fn a_frame_between_steps_shows_the_earlier_step() {
    let mut scene = Scene::<()>::new();
    scene.simulation_rate(10);
    let steps = scene.simulate(0_u32, |steps, _| *steps += 1);
    let mut animation = scene.compile([wait(s(1))]);

    for (time, expected) in [
        (ms(0), 0),
        (ms(99), 0),
        (ms(100), 1),
        (ms(150), 1),
        (s(1), 10),
    ] {
        animation.sample(time);
        assert_eq!(
            *animation.simulated(steps),
            expected,
            "at {time:?}"
        );
    }
}

#[test]
fn every_step_receives_the_scene_step_length() {
    let mut scene = Scene::<()>::new();
    scene.simulation_rate(4);
    let elapsed = scene.simulate(0.0, |elapsed, dt| {
        assert_eq!(dt, 0.25);
        *elapsed += dt;
    });
    let mut animation = scene.compile([wait(s(2))]);
    animation.sample(s(2));
    assert_eq!(*animation.simulated(elapsed), 2.0);
}

#[test]
fn sampling_past_the_end_holds_the_last_step() {
    let mut scene = Scene::<()>::new();
    let steps = scene.simulate(0_u32, |steps, _| *steps += 1);
    let mut animation = scene.compile([wait(s(1))]);
    animation.sample(s(1));
    let last = *animation.simulated(steps);
    assert_eq!(last, 60);
    animation.sample(s(5));
    assert_eq!(*animation.simulated(steps), last);
}

#[test]
fn a_scene_without_animations_keeps_the_initial_state() {
    let mut scene = Scene::<()>::new();
    let value = scene.simulate(7, |value, _| *value += 1);
    let mut animation = scene.compile([]);
    animation.sample(s(3));
    assert_eq!(*animation.simulated(value), 7);
}

/// A simulation may read another one: each step sees the other's state at
/// that step's time.
#[test]
fn a_simulation_can_read_another() {
    let mut scene = Scene::<()>::new();
    scene.simulation_rate(10);
    let velocity =
        scene.simulate(0.0, |velocity, dt| *velocity += 2.0 * dt);
    let position = scene.simulate_with(
        0.0,
        velocity,
        |position, velocity, dt| {
            *position += velocity * dt;
        },
    );
    let mut animation = scene.compile([wait(s(1))]);
    animation.sample(s(1));

    // Forward Euler: position(n) = Σ velocity(k) dt for k < n, with
    // velocity(k) = 2 k dt, so position(10) = 2 dt² · 45 = 0.9.
    assert!((*animation.simulated(velocity) - 2.0).abs() < 1e-12);
    assert!((*animation.simulated(position) - 0.9).abs() < 1e-12);
}

#[test]
fn several_inputs_arrive_as_a_tuple() {
    let wind = signal(1.0);
    let gravity = signal(-2.0);
    let mut scene = Scene::<()>::new();
    scene.simulation_rate(10);
    let speed = scene.simulate_with(
        0.0,
        (&wind, computed(&gravity, |gravity: f64| gravity * 2.0)),
        |speed, (wind, gravity), dt| *speed += (wind + gravity) * dt,
    );
    let mut animation = scene.compile([wait(s(1))]);
    animation.sample(s(1));
    assert!((*animation.simulated(speed) - -3.0).abs() < 1e-12);
}

#[test]
#[should_panic(expected = "at least one step per second")]
fn a_zero_rate_is_rejected() {
    Scene::<()>::new().simulation_rate(0);
}

#[test]
fn step_times_survive_long_scenes() {
    let mut scene = Scene::<()>::new();
    let steps = scene.simulate(0_u64, |steps, _| *steps += 1);
    let mut animation =
        scene.compile([wait(Duration::from_secs(600))]);
    animation.sample(Duration::from_secs(600));
    assert_eq!(*animation.simulated(steps), 36_000);
}
