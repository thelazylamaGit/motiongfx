//! A Lissajous table built with ordinary shapes, reactive inputs and one timeline.
//! The grid introduction plays once; diagonal draw/hold/erase waves then loop.
#![allow(clippy::needless_return)] // Explicit returns clarify multi-statement math.

use std::f64::consts::{FRAC_PI_4, TAU};
use std::time::Duration;

use motiongfx::prelude::{cs, ease, ms};
use motiongfx_2d::{
    Animation, Circle, Color, Line, Path, Point, Scene, Signal,
    colors::{TRANSPARENT, WHITE},
    computed, signal,
};
use vello_winit_motiongfx::{PlaybackMode, Window};

const N_X: usize = 12;
const N_Y: usize = 6;
const CELL_W: f64 = 110.0;
const CELL_H: f64 = 113.0;
const CURVE_R: f64 = 42.0;
const REF_R: f64 = 36.0;
const DRAW_DUR: Duration = cs(150);
const HOLD_DUR: Duration = cs(80);
const STAGGER: Duration = cs(8); // Delay between successive diagonals.
const CANVAS: (f64, f64) =
    ((N_X + 1) as f64 * CELL_W, (N_Y + 1) as f64 * CELL_H);
const GRID: Color = Color::from_rgb8(105, 105, 140);
const CROSSHAIR: Color = Color::from_rgb8(90, 90, 110);
const REFERENCE: Color = Color::from_rgb8(220, 220, 220);

// Row/column zero contain the reference circles. The canvas is centred, y up.
fn cell(column: usize, row: usize) -> Point {
    Point::new(
        (column as f64 + 0.5) * CELL_W - CANVAS.0 / 2.0,
        CANVAS.1 / 2.0 - (row as f64 + 0.5) * CELL_H,
    )
}

fn curve_id(column: usize, row: usize) -> usize {
    (row - 1) * N_X + column - 1
}

fn curve_point(column: usize, row: usize, angle: f64) -> Point {
    cell(column, row)
        + (
            CURVE_R * (column as f64 * angle + FRAC_PI_4).sin(),
            -CURVE_R * (row as f64 * angle).sin(),
        )
}

fn curve_color(column: usize) -> Color {
    let fraction = (column - 1) as f64 / (N_X - 1).max(1) as f64;
    Color::from_rgb8(220, (fraction * 130.0) as u8, 0)
}

// A reusable component: fixed ring/crosshairs plus a dot driven by one input.
fn reference_circle(
    scene: &mut Scene,
    center: Point,
    progress: &Signal<f64>,
    rate: f64,
    phase: f64,
) {
    scene.add(
        Circle::new(REF_R)
            .position(center)
            .fill(TRANSPARENT)
            .stroke(REFERENCE, 1.0),
    );
    scene.add(
        Line::new(center + (0.0, -REF_R), center + (0.0, REF_R))
            .stroke(CROSSHAIR, 0.5),
    );
    scene.add(
        Line::new(center + (-REF_R, 0.0), center + (REF_R, 0.0))
            .stroke(CROSSHAIR, 0.5),
    );
    scene.add(Circle::new(3.5).fill(WHITE).position(progress.map(
        move |progress| {
            let angle = rate * progress * TAU + phase;
            return center
                + (REF_R * angle.sin(), REF_R * angle.cos());
        },
    )));
}

fn build_animation() -> (Animation, Duration) {
    let mut scene = Scene::new();
    // Each curve has a visible range: reveal its end, hold, then advance its start.
    // Separate sources let reference dots exist before their curves in draw order.
    let ranges: Vec<_> =
        (0..N_X * N_Y).map(|_| (signal(0.0), signal(0.0))).collect();
    let top_left = Point::new(-CANVAS.0 / 2.0, CANVAS.1 / 2.0);
    let verticals: Vec<_> = (0..=N_X + 1)
        .map(|column| {
            let start = top_left + (column as f64 * CELL_W, 0.0);
            scene.add(
                Line::new(start, start + (0.0, -CANVAS.1))
                    .stroke(GRID, 0.5)
                    .reveal(0.0),
            )
        })
        .collect();
    let horizontals: Vec<_> = (0..=N_Y + 1)
        .map(|row| {
            let start = top_left + (0.0, -(row as f64) * CELL_H);
            scene.add(
                Line::new(start, start + (CANVAS.0, 0.0))
                    .stroke(GRID, 0.5)
                    .reveal(0.0),
            )
        })
        .collect();

    for column in 1..=N_X {
        reference_circle(
            &mut scene,
            cell(column, 0),
            &ranges[curve_id(column, 1)].1,
            column as f64,
            FRAC_PI_4,
        );
    }
    for row in 1..=N_Y {
        reference_circle(
            &mut scene,
            cell(0, row),
            &ranges[curve_id(1, row)].1,
            row as f64,
            0.0,
        );
    }
    for row in 1..=N_Y {
        for column in 1..=N_X {
            let (start, end) = &ranges[curve_id(column, row)];
            let samples = column.max(row) * 80 + 2;
            // Hide tiny remnants at both ends, matching the original demo.
            let opacity = computed!((start, end) =>
                if end - start < 0.01 || 1.0 - start < 0.01 { 0.0 } else { 1.0 }
            );
            scene.add(
                Path::new((0..=samples).map(|index| {
                    let angle = index as f64 / samples as f64 * TAU;
                    return curve_point(column, row, angle);
                }))
                .stroke(curve_color(column), 2.0)
                .start(start)
                .end(end)
                .opacity(opacity),
            );
        }
    }

    let mut loop_start = Duration::ZERO;
    let animation = scene.animate(|sequence| {
        sequence.defaults(cs(60), ease::cubic::ease_in_out);
        // Rows follow columns by 25ms; successive pairs start 50ms apart.
        loop_start = sequence.staggered(ms(50), |sequence| {
            for index in 0..verticals.len().max(horizontals.len()) {
                sequence.staggered(ms(25), |sequence| {
                    if let Some(column) = verticals.get(index) {
                        sequence.play(column.reveal.to(1.0));
                    }
                    if let Some(row) = horizontals.get(index) {
                        sequence.play(row.reveal.to(1.0));
                    }
                });
            }
        });

        // Diagonals stagger; curves within a diagonal run together.
        sequence.staggered(STAGGER, |sequence| {
            for diagonal in 2..=N_X + N_Y {
                sequence.parallel(|sequence| {
                    for column in 1..=N_X {
                        let Some(row) = diagonal.checked_sub(column)
                        else {
                            continue;
                        };
                        if !(1..=N_Y).contains(&row) {
                            continue;
                        }
                        let (start, end) =
                            &ranges[curve_id(column, row)];
                        sequence.chain(|sequence| {
                            sequence.play(end.to(1.0).over(DRAW_DUR));
                            sequence.wait(HOLD_DUR);
                            sequence
                                .play(start.to(1.0).over(DRAW_DUR));
                        });
                    }
                });
            }
        });
    });
    (animation, loop_start)
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let (animation, loop_start) = build_animation();
    Window::new("MotionGfx | Lissajous table", CANVAS)
        .playback_mode(PlaybackMode::LoopFrom(loop_start))
        .run(animation)
}

#[cfg(test)]
#[path = "support/lissajous_tests.rs"]
mod tests;
