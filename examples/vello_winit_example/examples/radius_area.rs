//! Motion Canvas's radius/area signal example, using native computed values.
//! Only radius is animated. Circle size, arrow endpoint, labels and their
//! positions all follow it automatically, including when seeking backwards.

use std::f64::consts::PI;

use motiongfx::prelude::{ease, s};
use vello_motiongfx::{Circle, Color, Line, Scene, Text, TextAlign};
use vello_winit_motiongfx::Window;

const SCALE: f64 = 100.0;
const RED: Color = Color::from_rgb8(225, 50, 56);
const INK: Color = Color::from_rgb8(36, 36, 36);

fn main() -> Result<(), winit::error::EventLoopError> {
    let mut scene = Scene::new();
    scene.set_background(Color::from_rgb8(250, 249, 246));
    let radius = scene.signal(3.0);
    let area = scene.computed(radius, |r| PI * r * r);
    let pixels = scene.computed(radius, |r| r * SCALE);
    let radius_end = scene.computed(pixels, |r| (r, 0.0));
    let radius_text =
        scene.computed(radius, |r| format!("r = {r:.2}").into());
    let area_text =
        scene.computed(area, |a| format!("A = {a:.2}").into());
    // Top-anchored labels get the source's 20px inset. Our canvas is y-up,
    // so the area label sits below the circle at negative y.
    let radius_label_position =
        scene.computed(pixels, |r| (r / 2.0, -20.0));
    let area_label_position =
        scene.computed(pixels, |r| (0.0, -r - 20.0));

    scene.add(Circle::new(pixels).fill(RED));
    scene.add(
        Line::new((0.0, 0.0), radius_end)
            .line_dash([20.0, 20.0])
            .start_arrow(true)
            .end_arrow(true)
            .end_offset(8.0)
            .stroke(INK, 8.0),
    );
    scene.add(
        Text::new(radius_text)
            .position(radius_label_position)
            .font_size(56.0)
            .align(TextAlign::Center)
            .fill(INK),
    );
    scene.add(
        Text::new(area_text)
            .position(area_label_position)
            .font_size(56.0)
            .align(TextAlign::Center)
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
