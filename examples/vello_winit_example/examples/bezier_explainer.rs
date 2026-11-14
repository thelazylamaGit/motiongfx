//! A quadratic Bézier lesson: create objects, describe relationships, animate.
//! Signals are writable timeline inputs; computed values are read-only math.
//! Object builders accept either fixed values or those typed sources. There
//! are no per-object update callbacks, custom World, or renderer in this file.

use motiongfx::prelude::{ease, ms, s};
use motiongfx_2d::{
    Circle, Color, Input, Line, LineHandle, Path, Point, Rectangle,
    Scene, Text, all, chain, computed, signal, wait,
};
use vello::kurbo::Rect;
use vello_winit_motiongfx::{PlaybackMode, Window};

const INK: Color = Color::from_rgb8(230, 235, 246);
const MUTED: Color = Color::from_rgb8(146, 157, 184);
const TEAL: Color = Color::from_rgb8(72, 222, 194);
const GOLD: Color = Color::from_rgb8(255, 197, 103);
const PINK: Color = Color::from_rgb8(248, 128, 170);

fn lerp(a: Point, b: Point, t: f64) -> Point {
    a + (b - a) * t
}

/// Reusable background geometry. Each line's reveal is animated directly,
/// without point-buffer reconstruction or a procedural update function.
fn grid(
    scene: &mut Scene,
    bounds: Rect,
    spacing: f64,
) -> Vec<LineHandle> {
    let mut lines = Vec::new();
    for column in 0..=(bounds.width() / spacing) as usize {
        let x = bounds.x0 + column as f64 * spacing;
        lines.push(
            scene.add(
                Line::new((x, bounds.y1), (x, bounds.y0))
                    .stroke(MUTED, 1.0)
                    .opacity(0.13)
                    .reveal(0.0),
            ),
        );
    }
    for row in 0..=(bounds.height() / spacing) as usize {
        let y = bounds.y1 - row as f64 * spacing;
        lines.push(
            scene.add(
                Line::new((bounds.x0, y), (bounds.x1, y))
                    .stroke(MUTED, 1.0)
                    .opacity(0.13)
                    .reveal(0.0),
            ),
        );
    }
    lines
}

fn build_animation() -> motiongfx_2d::CompiledScene {
    let mut scene = Scene::new();
    let guides = signal(0.0);
    let t = signal(0.0);
    let bend = signal(0.0);

    // One shared calculation for each construction point. Positive y is up.
    let p0 = Point::new(-400.0, -150.0);
    let p2 = Point::new(400.0, -150.0);
    let p1 = bend.map(|bend| Point::new(0.0, 170.0 - bend));
    let a_position = computed!((p1, t) => lerp(p0, p1, t));
    let b_position = computed!((p1, t) => lerp(p1, p2, t));
    let q_position = computed!((a_position, b_position, t) => lerp(a_position, b_position, t));
    let polygon_opacity = guides.map(|opacity| opacity * 0.6);
    let halo_opacity = guides.map(|opacity| opacity * 0.16);

    scene.add(
        Rectangle::new(960.0, 440.0)
            .position((0.0, 10.0))
            .fill(Color::from_rgb8(22, 27, 43)),
    );
    scene.add(
        Text::new("A CURVE FROM THREE POINTS")
            .position((-480.0, 358.0))
            .font_size(28.0)
            .fill(INK),
    );
    scene.add(
        Text::new("QUADRATIC BEZIER / DE CASTELJAU CONSTRUCTION")
            .position((-480.0, 304.0))
            .font_size(14.0)
            .fill(MUTED),
    );
    let chapter = scene.add(
        Text::new("01  BUILD THE GRID AND CONTROL POLYGON")
            .position((-480.0, 266.0))
            .font_size(14.0)
            .fill(TEAL),
    );
    let grid_lines = grid(
        &mut scene,
        Rect::new(-480.0, -210.0, 480.0, 230.0),
        40.0,
    );

    scene.add(
        Line::new(p0, &p1)
            .stroke(MUTED, 1.5)
            .opacity(&polygon_opacity),
    );
    scene.add(
        Line::new(&p1, p2)
            .stroke(MUTED, 1.5)
            .opacity(polygon_opacity),
    );
    let curve = scene
        .add(Path::quadratic(p0, &p1, p2).stroke(TEAL, 4.0).end(0.0));
    scene.add(
        Line::new(&a_position, &b_position)
            .stroke(GOLD, 2.0)
            .opacity(&guides),
    );
    let a = scene.add(
        Circle::new(5.0)
            .position(&a_position)
            .fill(GOLD)
            .opacity(&guides),
    );
    let b = scene.add(
        Circle::new(5.0)
            .position(&b_position)
            .fill(GOLD)
            .opacity(&guides),
    );

    // The array mixes fixed and computed positions through one typed Input.
    let controls =
        [Input::from(p0), (&p1).into(), p2.into()].map(|position| {
            scene.add(
                Circle::new(6.0)
                    .position(position)
                    .fill(INK)
                    .opacity(&guides),
            )
        });
    for (name, control) in
        ["P0", "P1", "P2"].into_iter().zip(controls)
    {
        scene.add(
            Text::new(name)
                .relative_to(control)
                .position((12.0, 25.0))
                .font_size(14.0)
                .fill(INK)
                .opacity(&guides),
        );
    }
    scene.add(
        Text::new("A")
            .relative_to(a)
            .position((-22.0, 25.0))
            .font_size(14.0)
            .fill(GOLD)
            .opacity(&guides),
    );
    scene.add(
        Text::new("B")
            .relative_to(b)
            .position((12.0, -14.0))
            .font_size(14.0)
            .fill(GOLD)
            .opacity(&guides),
    );
    scene.add(
        Circle::new(16.0)
            .position(&q_position)
            .fill(PINK)
            .opacity(halo_opacity),
    );
    scene.add(
        Circle::new(7.0)
            .position(q_position)
            .fill(PINK)
            .opacity(&guides),
    );

    for (content, y, color) in [
        ("A = LERP(P0, P1, T)", -244.0, GOLD),
        ("B = LERP(P1, P2, T)", -272.0, GOLD),
        ("Q = LERP(A, B, T)", -300.0, PINK),
    ] {
        scene.add(
            Text::new(content)
                .position((-480.0, y))
                .font_size(14.0)
                .fill(color),
        );
    }
    let time = scene.time();
    let readout = computed!((t, time) => format!("T = {t:.2}  TIME {:.2}S", time.as_secs_f64()).into());
    scene.add(
        Text::new(readout)
            .position((100.0, -244.0))
            .font_size(14.0)
            .fill(INK),
    );
    scene.add(
        Text::new("DRAG TO SEEK / SPACE TO PAUSE")
            .position((100.0, -286.0))
            .font_size(14.0)
            .fill(MUTED),
    );

    // Chapter changes are timeline events, not a second set of time thresholds.
    scene.defaults(s(1), ease::linear);
    scene.compile([
        all(grid_lines.iter().map(|line| {
            line.reveal()
                .to(1.0)
                .over(ms(900))
                .ease(ease::cubic::ease_in_out)
        })),
        guides.to(1.0).over(ms(600)),
        chapter.set_content("02  SLIDE TWO POINTS - TRACE A CURVE"),
        all([t.to(1.0).over(s(4)), curve.end().to(1.0).over(s(4))]),
        t.to(0.5).ease(ease::cubic::ease_in_out),
        chapter.set_content("03  MOVE P1 - THE WHOLE CURVE RESPONDS"),
        chain([200.0, -240.0, 40.0].map(|offset| {
            bend.offset(offset)
                .over(ms(1800))
                .ease(ease::cubic::ease_in_out)
        })),
        chapter
            .set_content("04  SAME MATH AT EVERY TIMELINE POSITION"),
        t.to(1.0).over(ms(1500)).ease(ease::cubic::ease_in_out),
        wait(s(1)),
    ])
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let animation = build_animation();

    Window::new(
        "MotionGfx | A curve from three points",
        (1120.0, 800.0),
    )
    .playback_mode(PlaybackMode::PingPong)
    .run(animation)
}

#[cfg(test)]
#[path = "support/headless.rs"]
mod headless;

#[test]
fn builds_and_samples_headlessly() {
    headless::check(build_animation());
}
