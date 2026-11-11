//! Manim's SineCurveUnitCircle with native, typed property bindings.
//! Objects/styles are created once. MotionGfx animates the curve's reveal;
//! Rust bindings calculate the dot and connecting endpoints after sampling.

use std::f64::consts::TAU;

use motiongfx::prelude::{ease, ms, s};
use vello_motiongfx::{
    Circle, Color, Line, Path, Point, Scene, Text,
    colors::{BLUE, TRANSPARENT, WHITE},
};
use vello_winit_motiongfx::Window;

const TURNS: f64 = 8.5 * 0.25;
const SEGMENTS: usize = 1088;
const ORIGIN: Point = Point::new(-320.0, 0.0);
const DOT_COLOR: Color = Color::from_rgb8(247, 217, 111);
const CONNECTOR_COLOR: Color = Color::from_rgb8(255, 241, 182);
const CURVE_COLOR: Color = Color::from_rgb8(244, 211, 69);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scene = Scene::new();

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

    // Create the moving objects with explicit initial positions and styles.
    let initial_dot = Point::new(-240.0, 0.0);
    let radius =
        scene.add(Line::new(ORIGIN, initial_dot).stroke(BLUE, 4.0));
    let connector = scene.add(
        Line::new(initial_dot, initial_dot)
            .stroke(CONNECTOR_COLOR, 2.0),
    );

    // Generate the entire sine path ONCE. Only its visible prefix changes.
    let curve = scene.add(
        Path::new((0..=SEGMENTS).map(|index| {
            let turns = TURNS * index as f64 / SEGMENTS as f64;
            Point::new(
                -240.0 + turns * 320.0,
                80.0 * (turns * TAU).sin(),
            )
        }))
        .stroke(CURVE_COLOR, 4.0)
        .end(0.0),
    );
    let dot = scene
        .add(Circle::new(6.4).position(initial_dot).fill(DOT_COLOR));

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

    // These closures run when their sampled inputs change, not during baking.
    // `reveal` is curve.end's current 0..1 value, not a hidden global clock.
    scene.bind(dot.position, curve.end, |reveal| {
        let angle = reveal * TURNS * TAU;
        Point::new(ORIGIN.x + 80.0 * angle.cos(), 80.0 * angle.sin())
    });
    scene.bind(radius.end, dot.position, |position| position);
    scene.bind(connector.start, dot.position, |position| position);
    // Multiple inputs: x follows reveal progress, y follows the moving dot.
    scene.bind(
        connector.end,
        (curve.end, dot.position.y),
        |(reveal, y)| Point::new(-240.0 + reveal * TURNS * 320.0, y),
    );

    // The same sampled reveal drives all motion, including backward seeks.
    // `try_animate` reports binding cycles/writer conflicts before playback.
    let animation = scene.try_animate(|sequence| {
        sequence.play(
            curve.end.to(1.0).over(ms(8500)).ease(ease::linear),
        );
        sequence.wait(s(1));
    })?;

    Window::new(
        "MotionGfx | Unit circle and sine curve",
        (1120.0, 600.0),
    )
    .run(animation)?;
    Ok(())
}
