//! Manim's MovingAround, using typed native property handles.
//! Creation happens once; the animation closure builds and bakes the timeline.

use motiongfx::prelude::s;
use vello_motiongfx::{Color, Drawing, Rectangle, Size, smooth};
use vello_winit_motiongfx::Window;

fn main() -> Result<(), winit::error::EventLoopError> {
    let blue = Color::from_rgb8(88, 196, 221);
    let orange = Color::from_rgb8(255, 134, 47);
    let mut drawing = Drawing::new();

    // Explicitly create a 160px square. Fill and stroke are independent.
    let square = drawing.rectangle(Rectangle {
        size: Size::new(160.0, 160.0),
        fill: blue,
        stroke: blue,
        stroke_width: 4.0,
        ..Rectangle::default()
    });

    let animation = drawing.animate(|sequence| {
        sequence.defaults(s(1), smooth);
        sequence.play(square.position.offset((-80.0, 0.0)));
        sequence.play(square.fill.to(orange));
        sequence.play(square.scale.multiply(0.3));
        sequence.play(square.rotation.offset(0.4)); // Radians, counterclockwise.
    });

    // Window size is separate from duration, which comes from the timeline.
    Window::new("MotionGfx | Moving square", Size::new(900.0, 540.0))
        .run(animation)
}
