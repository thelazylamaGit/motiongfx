//! A quadratic Bézier lesson using ordinary Rust update callbacks.
//!
//! Read in order: math -> objects -> updates -> timeline. Objects and styles
//! are created once. Updates run after each absolute-time timeline sample,
//! in registration order, including backward seeks. No bindings are used.
//! Drag the shared playback strip to seek; Space pauses/resumes playback.

use motiongfx::prelude::{ease, ms, s};
use vello_motiongfx::{
    Circle, Color, Line, Path, Point, Rectangle, Scene, Text,
};
use vello_winit_motiongfx::{PlaybackMode, Window};

const INK: Color = Color::from_rgb8(230, 235, 246);
const MUTED: Color = Color::from_rgb8(146, 157, 184);
const TEAL: Color = Color::from_rgb8(72, 222, 194);
const GOLD: Color = Color::from_rgb8(255, 197, 103);
const PINK: Color = Color::from_rgb8(248, 128, 170);

fn lerp(a: Point, b: Point, t: f64) -> Point {
    a + (b - a) * t
}

/// The middle point moves down as bend increases; our canvas has y upward.
fn control_points(bend: f64) -> [Point; 3] {
    [
        Point::new(-400.0, -150.0),
        Point::new(0.0, 170.0 - bend),
        Point::new(400.0, -150.0),
    ]
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let mut scene = Scene::new();

    // Standalone animated numbers: no dummy graphics objects or custom World.
    let grid = scene.value(0.0);
    let guides = scene.value(0.0);
    let t = scene.value(0.0);
    let bend = scene.value(0.0);

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
        Text::new("")
            .position((-480.0, 266.0))
            .font_size(14.0)
            .fill(TEAL),
    );

    // Each grid line starts collapsed at its first endpoint.
    let verticals: [_; 25] = std::array::from_fn(|column| {
        let start = Point::new(-480.0 + column as f64 * 40.0, 230.0);
        scene.add(
            Line::new(start, start).stroke(MUTED, 1.0).opacity(0.13),
        )
    });
    let horizontals: [_; 12] = std::array::from_fn(|row| {
        let start = Point::new(-480.0, 230.0 - row as f64 * 40.0);
        scene.add(
            Line::new(start, start).stroke(MUTED, 1.0).opacity(0.13),
        )
    });

    let [p0, p1, p2] = control_points(0.0);
    let polygon = [
        scene.add(Line::new(p0, p1).stroke(MUTED, 1.5).opacity(0.0)),
        scene.add(Line::new(p1, p2).stroke(MUTED, 1.5).opacity(0.0)),
    ];
    // An exact quadratic replaces the old 180-segment approximation.
    let curve = scene
        .add(Path::quadratic(p0, p1, p2).stroke(TEAL, 4.0).end(0.0));
    let construction_line =
        scene.add(Line::new(p0, p1).stroke(GOLD, 2.0).opacity(0.0));
    let a = scene
        .add(Circle::new(5.0).position(p0).fill(GOLD).opacity(0.0));
    let b = scene
        .add(Circle::new(5.0).position(p1).fill(GOLD).opacity(0.0));
    let controls = [p0, p1, p2].map(|point| {
        scene.add(
            Circle::new(6.0).position(point).fill(INK).opacity(0.0),
        )
    });
    // Anchored labels inherit position; no separate position updates needed.
    let control_labels = std::array::from_fn::<_, 3, _>(|index| {
        scene.add(
            Text::new(format!("P{index}"))
                .relative_to(controls[index])
                .position((12.0, 25.0))
                .font_size(14.0)
                .fill(INK)
                .opacity(0.0),
        )
    });
    let a_label = scene.add(
        Text::new("A")
            .relative_to(a)
            .position((-22.0, 25.0))
            .font_size(14.0)
            .fill(GOLD)
            .opacity(0.0),
    );
    let b_label = scene.add(
        Text::new("B")
            .relative_to(b)
            .position((12.0, -14.0))
            .font_size(14.0)
            .fill(GOLD)
            .opacity(0.0),
    );
    let halo = scene.add(Circle::new(16.0).fill(PINK).opacity(0.0));
    let q = scene.add(Circle::new(7.0).fill(PINK).opacity(0.0));

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
    let readout = scene.add(
        Text::new("")
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

    // DIRECT UPDATES. Arguments are targets, sampled sources, then a function.
    // Inside each function the targets are actual mutable objects, not handles.
    scene.update(verticals, grid, |lines, amount| {
        for line in lines {
            line.end = line.start + (0.0, -440.0 * amount);
        }
    });
    scene.update(horizontals, grid, |lines, amount| {
        for line in lines {
            line.end = line.start + (960.0 * amount, 0.0);
        }
    });
    scene.update(
        controls,
        (bend, guides),
        |dots, (bend, opacity)| {
            for (dot, point) in
                dots.into_iter().zip(control_points(bend))
            {
                dot.position = point;
                dot.opacity = opacity;
            }
        },
    );
    scene.update(
        polygon,
        (bend, guides),
        |lines, (bend, opacity)| {
            let [p0, p1, p2] = control_points(bend);
            for (line, (start, end)) in
                lines.into_iter().zip([(p0, p1), (p1, p2)])
            {
                line.start = start;
                line.end = end;
                line.opacity = opacity * 0.6;
            }
        },
    );
    scene.update(curve, bend, |curve, bend| {
        let [p0, p1, p2] = control_points(bend);
        // Same points reuse geometry. Only a changed bend edits the path.
        curve.set_quadratic(p0, p1, p2);
    });
    scene.update(
        (a, b, halo, q),
        (bend, t, guides),
        |(a, b, halo, q), (bend, t, opacity)| {
            let [p0, p1, p2] = control_points(bend);
            // De Casteljau: interpolate twice, then interpolate those two results.
            a.position = lerp(p0, p1, t);
            b.position = lerp(p1, p2, t);
            let curve_point = lerp(a.position, b.position, t);
            q.position = curve_point;
            halo.position = curve_point;
            a.opacity = opacity;
            b.opacity = opacity;
            q.opacity = opacity;
            halo.opacity = opacity * 0.16;
        },
    );
    // Registration order is explicit: A/B were updated above before this read.
    scene.update(
        construction_line,
        (a.position, b.position, guides),
        |line, (a, b, opacity)| {
            line.start = a;
            line.end = b;
            line.opacity = opacity;
        },
    );
    scene.update(
        [
            control_labels[0],
            control_labels[1],
            control_labels[2],
            a_label,
            b_label,
        ],
        guides,
        |labels, opacity| {
            for label in labels {
                label.opacity = opacity;
            }
        },
    );
    let time = scene.time();
    scene.update(chapter, time, |chapter, time| {
        let text = match time.as_secs_f64() {
            seconds if seconds < 1.5 => {
                "01  BUILD THE GRID AND CONTROL POLYGON"
            }
            seconds if seconds < 6.5 => {
                "02  SLIDE TWO POINTS - TRACE A CURVE"
            }
            seconds if seconds < 11.9 => {
                "03  MOVE P1 - THE WHOLE CURVE RESPONDS"
            }
            _ => "04  SAME MATH AT EVERY TIMELINE POSITION",
        };
        if chapter.content.as_ref() != text {
            chapter.content = text.into();
        }
    });
    // Keep text formatting out of repeated samples with identical readouts.
    let mut last_readout = None;
    scene.update(readout, (t, time), move |readout, (t, time)| {
        let key = (
            (t * 100.0).round() as i64,
            (time.as_secs_f64() * 100.0).round() as u64,
        );
        if last_readout != Some(key) {
            readout.content = format!(
                "T = {t:.2}  TIME {:.2}S",
                time.as_secs_f64()
            )
            .into();
            last_readout = Some(key);
        }
    });

    // TIMELINE. Same 14.4s lesson; callback functions do not schedule actions.
    let animation = scene.animate(|sequence| {
        sequence.defaults(s(1), ease::linear);
        sequence.play(
            grid.to(1.0).over(ms(900)).ease(ease::cubic::ease_in_out),
        );
        sequence.play(guides.to(1.0).over(ms(600)));
        sequence.parallel(|sequence| {
            sequence.play(t.to(1.0).over(s(4)));
            sequence.play(curve.end.to(1.0).over(s(4)));
        });
        sequence.play(t.to(0.5).ease(ease::cubic::ease_in_out));
        for offset in [200.0, -240.0, 40.0] {
            sequence.play(
                bend.offset(offset)
                    .over(ms(1800))
                    .ease(ease::cubic::ease_in_out),
            );
        }
        sequence.play(
            t.to(1.0).over(ms(1500)).ease(ease::cubic::ease_in_out),
        );
        sequence.wait(s(1));
    });

    Window::new(
        "MotionGfx | A curve from three points",
        (1120.0, 800.0),
    )
    .playback_mode(PlaybackMode::PingPong)
    .run(animation)
}
