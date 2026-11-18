//! Two simulations: Manim's `RotationUpdater` and a ball bouncing on a
//! floor. Both advance in fixed steps from their previous state, run once
//! when the scene compiles, so the preview scrubs them in either direction.
//!
//! - Left: a yellow line turns counterclockwise for two seconds, back for
//!   two, then stops. Manim attaches and removes updaters; here the
//!   timeline switches a `rate` signal that every step reads.
//! - Right: a ball falls under gravity and keeps 80% of its speed at each
//!   bounce. It reads nothing from the timeline.

use motiongfx::prelude::{ms, s};
use motiongfx_2d::{
    Circle, Line, Point, Scene, all, chain,
    colors::{ORANGE, WHITE, YELLOW},
    signal, wait,
};
use vello_winit_example::Window;

/// Pivot of the two rotation lines.
const PIVOT: Point = Point::new(-220.0, 0.0);
/// One Manim unit, in canvas pixels.
const UNIT: f64 = 80.0;

/// Downward acceleration, in canvas pixels per second squared.
const GRAVITY: f64 = 980.0;
const BALL_RADIUS: f64 = 20.0;
const BALL_X: f64 = 220.0;
/// Height of the floor line.
const FLOOR: f64 = -200.0;
/// Fraction of its speed the ball keeps after each bounce.
const RESTITUTION: f64 = 0.8;

/// Everything the ball simulation remembers from one step to the next.
#[derive(Clone, PartialEq)]
struct Ball {
    height: f64,
    velocity: f64,
}

fn build_animation() -> motiongfx_2d::CompiledScene {
    let mut scene = Scene::new();

    // Rotation updater. `rate` is in radians per second; 0 means no
    // updater is attached.
    let rate = signal(0.0);
    let angle = scene.simulate_with(0.0, &rate, |angle, rate, dt| {
        *angle += rate * dt;
    });
    // Both lines start at the pivot and point left (Manim's `LEFT`).
    scene.add(
        Line::new(PIVOT, (PIVOT.x - UNIT, PIVOT.y))
            .stroke(WHITE, 4.0),
    );
    scene.add(
        Line::new(
            PIVOT,
            angle.map(|angle| {
                Point::new(
                    PIVOT.x - UNIT * angle.cos(),
                    PIVOT.y - UNIT * angle.sin(),
                )
            }),
        )
        .stroke(YELLOW, 4.0),
    );

    // Bouncing ball, dropped from rest.
    let ball = scene.simulate(
        Ball {
            height: 200.0,
            velocity: 0.0,
        },
        |ball, dt| {
            ball.velocity -= GRAVITY * dt;
            ball.height += ball.velocity * dt;
            // A step can carry the ball into the floor: put it back on the
            // floor and send it up again, slower.
            let resting = FLOOR + BALL_RADIUS;
            if ball.height < resting {
                ball.height = resting;
                ball.velocity = -ball.velocity * RESTITUTION;
            }
        },
    );
    scene.add(
        Line::new((BALL_X - 120.0, FLOOR), (BALL_X + 120.0, FLOOR))
            .stroke(WHITE, 2.0),
    );
    scene.add(
        Circle::new(BALL_RADIUS)
            .position(
                ball.map(|ball| Point::new(BALL_X, ball.height)),
            )
            .fill(ORANGE),
    );

    // The scene's length is how long the simulations run: the rotation's
    // 4.5 seconds, with the ball bouncing for 6.
    scene.compile([all([
        chain([
            rate.set(1.0), // add updater_forth
            wait(s(2)),
            rate.set(-1.0), // swap to updater_back
            wait(s(2)),
            rate.set(0.0), // remove the updater
            wait(ms(500)),
        ]),
        wait(s(6)),
    ])])
}

fn main() -> Result<(), vello_winit_example::RunError> {
    Window::new("MotionGfx | Simulations", (900.0, 540.0))
        .run(build_animation())
}

#[cfg(test)]
#[path = "support/headless.rs"]
mod headless;

#[test]
fn builds_and_samples_headlessly() {
    headless::check(build_animation());
}
