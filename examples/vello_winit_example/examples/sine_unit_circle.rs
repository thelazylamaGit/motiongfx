//! Manim's SineCurveUnitCircle with shared native signal/computed inputs.
//! One progress signal drives the orbit, connecting lines and curve reveal.
//! Objects and the full sine path are created once; sampling is seekable.
#![allow(clippy::needless_return)] // Explicit returns clarify multi-statement math.

use std::f64::consts::TAU;

use motiongfx::prelude::{ease, ms, s};
use motiongfx_2d::{
    Circle, Color, Line, Path, Point, Scene, Text,
    colors::{BLUE, TRANSPARENT, WHITE},
    computed, signal, wait,
};
use vello_winit_motiongfx::Window;

const TURNS: f64 = 8.5 * 0.25;
const SEGMENTS: usize = 1088;
const ORIGIN: Point = Point::new(-320.0, 0.0);
const DOT_COLOR: Color = Color::from_rgb8(247, 217, 111);
const CONNECTOR_COLOR: Color = Color::from_rgb8(255, 241, 182);
const CURVE_COLOR: Color = Color::from_rgb8(244, 211, 69);

fn build_animation()
-> Result<motiongfx_2d::CompiledScene, motiongfx_2d::SceneError> {
    let mut scene = Scene::new();
    let progress = signal(0.0);
    let orbit = progress.map(|progress| {
        let angle = progress * TURNS * TAU;
        return Point::new(
            ORIGIN.x + 80.0 * angle.cos(),
            80.0 * angle.sin(),
        );
    });
    let curve_tip = computed!((progress, orbit) =>
        Point::new(-240.0 + progress * TURNS * 320.0, orbit.y)
    );

    // The original uses mathematical units; each unit here is 80px.
    scene.add(
        Line::new((-480.0, 0.0), (480.0, 0.0)).stroke(WHITE, 2.0),
    );
    scene.add(
        Line::new((-320.0, -160.0), (-320.0, 160.0))
            .stroke(WHITE, 2.0),
    );
    scene.add(
        Circle::new(80.0)
            .position(ORIGIN)
            .fill(TRANSPARENT)
            .stroke(WHITE, 4.0),
    );

    // Declare relationships when creating objects. Lines draw behind the dot.
    scene.add(Line::new(ORIGIN, &orbit).stroke(BLUE, 4.0));
    scene.add(
        Line::new(&orbit, &curve_tip).stroke(CONNECTOR_COLOR, 2.0),
    );

    // Generate the entire sine path ONCE. Only its visible prefix changes.
    scene.add(
        Path::new((0..=SEGMENTS).map(|index| {
            let turns = TURNS * index as f64 / SEGMENTS as f64;
            Point::new(
                -240.0 + turns * 320.0,
                80.0 * (turns * TAU).sin(),
            )
        }))
        .stroke(CURVE_COLOR, 4.0)
        .end(&progress),
    );
    scene.add(Circle::new(6.4).position(&orbit).fill(DOT_COLOR));

    for (index, content) in
        ["PI", "2 PI", "3 PI", "4 PI"].iter().enumerate()
    {
        scene.add(
            Text::new(*content)
                .position((-92.0 + index as f64 * 160.0, -18.0))
                .font_size(14.0)
                .fill(WHITE),
        );
    }

    // Shared calculations run when inputs change, including backward seeks.
    // `try_compile` reports binding cycles/writer conflicts before playback.
    scene.try_compile([
        progress.to(1.0).over(ms(8500)).ease(ease::linear),
        wait(s(1)),
    ])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let animation = build_animation()?;

    Window::new(
        "MotionGfx | Unit circle and sine curve",
        (1120.0, 600.0),
    )
    .run(animation)?;
    Ok(())
}

#[cfg(test)]
#[path = "support/headless.rs"]
mod headless;

#[test]
fn builds_and_samples_headlessly() {
    headless::check(build_animation().expect("valid scene"));
}
