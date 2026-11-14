//! Manim's MovingAround, using typed native property handles.
//! Creation happens once; the animation closure builds and bakes the timeline.

use motiongfx::time::ms;
use motiongfx_2d::{
    Rectangle, Scene, Spring,
    colors::{BLUE, ORANGE},
};
use vello_winit_motiongfx::Window;

fn main() -> Result<(), winit::error::EventLoopError> {
    let mut scene = Scene::new();

    // Explicitly create a 160px square. Fill and stroke are independent.
    let square = scene.add(
        Rectangle::new(160.0, 160.0).fill(BLUE).stroke(BLUE, 4.0),
    );

    let spring = Spring::timed(ms(650))
        .bounce(0.65)
        .rest_delta(0.001)
        .rest_speed(0.01);

    let animation = scene.animate(|sequence| {
        sequence.play(square.shift(-80.0, 0.0).spring(spring));
        // Colors tween: spring overshoot would leave the valid color range.
        sequence.play(square.fill_to(ORANGE).over(spring.duration()));
        sequence.play(square.scale_by(0.3).spring(spring));
        sequence.play(square.rotate(0.4).spring(spring));
    });

    // Window size is separate from duration, which comes from the timeline.
    Window::new("MotionGfx | Moving square", (900.0, 540.0))
        .run(animation)
}
