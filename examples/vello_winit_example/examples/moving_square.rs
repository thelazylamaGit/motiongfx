//! Manim's MovingAround, using typed native property handles.
//! Creation happens once; the animation closure builds and bakes the timeline.

use motiongfx_2d::{
    Rectangle, Scene,
    colors::{BLUE, ORANGE},
};
use vello_winit_motiongfx::Window;

fn main() -> Result<(), winit::error::EventLoopError> {
    let mut scene = Scene::new();

    // Explicitly create a 160px square. Fill and stroke are independent.
    let square = scene.add(
        Rectangle::new(160.0, 160.0).fill(BLUE).stroke(BLUE, 4.0),
    );

    let animation = scene.animate(|sequence| {
        sequence.play(square.position.offset((-80.0, 0.0)));
        sequence.play(square.fill.to(ORANGE));
        sequence.play(square.scale.multiply(0.3));
        sequence.play(square.rotation.offset(0.4)); // Radians, counterclockwise.
    });

    // Window size is separate from duration, which comes from the timeline.
    Window::new("MotionGfx | Moving square", (900.0, 540.0))
        .run(animation)
}
