//! Manim's MovingAround, using typed native property handles.
//! Objects are created once; `compile` runs the animations in order.

use motiongfx::time::ms;
use motiongfx_2d::{
    Rectangle, Scene, Spring,
    colors::{BLUE, ORANGE},
};
use vello_winit_motiongfx::Window;

fn build_animation() -> motiongfx_2d::CompiledScene {
    let mut scene = Scene::new();

    // Explicitly create a 160px square. Fill and stroke are independent.
    let square = scene.add(
        Rectangle::new(160.0, 160.0).fill(BLUE).stroke(BLUE, 4.0),
    );

    let spring = Spring::timed(ms(650))
        .bounce(0.65)
        .rest_delta(0.001)
        .rest_speed(0.01);

    scene.compile([
        square.shift(-80.0, 0.0).spring(spring),
        // The colour tweens over the spring's duration, without overshoot.
        square.fill_to(ORANGE).over(spring.duration()),
        square.scale_by(0.3).spring(spring),
        square.rotate(0.4).spring(spring),
    ])
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let animation = build_animation();

    // Window size is separate from duration, which comes from the timeline.
    Window::new("MotionGfx | Moving square", (900.0, 540.0))
        .run(animation)
}

#[cfg(test)]
#[path = "support/headless.rs"]
mod headless;

#[test]
fn builds_and_samples_headlessly() {
    headless::check(build_animation());
}
