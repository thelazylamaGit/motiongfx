//! Motion Canvas's radius/area signal example, using native computed values.
//! Only radius is animated. Circle size, arrow endpoint, labels and their
//! positions all follow it automatically, including when seeking backwards.

use std::f64::consts::PI;

use motiongfx::prelude::{ease, s};
use vello_motiongfx::{
    Circle, Color, Line, Scene, Text, TextAlign, Triangle, signal,
};
use vello_winit_motiongfx::Window;

const SCALE: f64 = 100.0;
const RED: Color = Color::from_rgb8(225, 50, 56);
const INK: Color = Color::from_rgb8(36, 36, 36);

fn main() -> Result<(), winit::error::EventLoopError> {
    let mut scene = Scene::new();
    scene.set_background(Color::from_rgb8(250, 249, 246));
    let radius = signal(3.0);
    let area = radius.map(|r| PI * r * r);
    let text_style =
        Text::new("").font_size(56.0).align(TextAlign::Center);

    // Inline calculations retain their source without borrowing the scene.
    scene.add(Circle::new(radius.map(|r| r * SCALE)).fill(RED));

    scene.add(
        Line::new((0.0, 0.0), radius.map(|r| (r * SCALE, 0.0)))
            .line_dash([20.0, 20.0])
            .start_tip(Triangle::new().size(24.0).fill(INK))
            .end_tip(Triangle::new().size(24.0).fill(INK))
            .end_offset(8.0)
            .stroke(INK, 8.0),
    );

    // Labels are top-anchored with a 20px inset. Negative y moves downward.
    scene.add(
        text_style
            .clone()
            .content(radius.map(|r| format!("r = {r:.2}").into()))
            .position(radius.map(|r| (r * SCALE / 2.0, -20.0)))
            .fill(INK),
    );

    scene.add(
        text_style
            .content(area.map(|a| format!("A = {a:.2}").into()))
            .position(radius.map(|r| (0.0, -r * SCALE - 20.0)))
            .fill(RED),
    );

    let animation = scene.animate(|sequence| {
        // Motion Canvas's default signal tween easing is cubic in/out.
        sequence.defaults(s(2), ease::cubic::ease_in_out);
        sequence.play(radius.to(4.0));
        sequence.play(radius.to(3.0));
        sequence.wait(s(1));
    });

    Window::new(
        "MotionGfx | Radius and area signals",
        (1280.0, 1000.0),
    )
    .run(animation)
}
