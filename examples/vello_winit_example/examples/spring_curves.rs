//! Edit the experiments below, then compare physical, visual and timed springs.
//! Each experiment runs all three balls together. Settings morph between runs;
//! they do not modify a spring already in flight. Space pauses; drag to seek.
#![allow(clippy::needless_return)]

use motiongfx::prelude::{ease, ms};
use motiongfx_2d::{
    BezPath, Circle, Color, CompiledScene, Line, Path, Point, Scene,
    Scene2dExt, Spring, SpringCurve, Text, TextAlign, TextStyle, all,
    colors::{TRANSPARENT, WHITE},
    computed, signal, wait,
};
use std::{sync::Arc, time::Duration};
use vello_winit_motiongfx::Window;

const WIDTH: f64 = 820.0;
const HEIGHT: f64 = 50.0;
const TRAVEL: f64 = 180.0;
const CANVAS: (f64, f64) = (1100.0, 1000.0);
const MUTED: Color = Color::from_rgb8(145, 151, 170);
const GRID: Color = Color::from_rgb8(48, 53, 67);

struct Experiment {
    name: &'static str,
    physical: Spring,
    visual: Spring,
    timed: Spring,
}

// Edit/add experiments here. These are ordinary spring settings, so every
// option is available. Matching visual/timed durations make their difference clear.
fn experiments() -> Vec<Experiment> {
    vec![
        Experiment {
            name: "01 / ELASTIC",
            physical: Spring::physical()
                .stiffness(120.0)
                .damping(7.0)
                .mass(1.0)
                .rest_delta(0.001)
                .rest_speed(0.01)
                .into(),
            visual: Spring::visual(ms(650))
                .bounce(0.7)
                .rest_delta(0.001)
                .rest_speed(0.01)
                .into(),
            timed: Spring::timed(ms(650))
                .bounce(0.65)
                .rest_delta(0.001)
                .rest_speed(0.01)
                .into(),
        },
        Experiment {
            name: "02 / FAST AND TIGHT",
            physical: Spring::physical()
                .stiffness(420.0)
                .damping(32.0)
                .mass(0.7)
                .into(),
            visual: Spring::visual(ms(300))
                .bounce(0.15)
                .rest_delta(0.0005)
                .into(),
            timed: Spring::timed(ms(300))
                .bounce(0.15)
                .rest_delta(0.0005)
                .into(),
        },
        Experiment {
            name: "03 / HEAVY AND OVERDAMPED",
            physical: Spring::physical()
                .stiffness(90.0)
                .damping(42.0)
                .mass(2.0)
                .into(),
            visual: Spring::visual(ms(900))
                .bounce(-0.6)
                .rest_speed(0.02)
                .into(),
            timed: Spring::timed(ms(900))
                .bounce(-0.6)
                .rest_speed(0.02)
                .into(),
        },
    ]
}

// Shared axes show seconds, not normalized duration: the timed spring's short
// tail can be compared directly with a physical or visual spring's longer tail.
fn graph(
    curve: SpringCurve,
    seconds: f64,
    origin: Point,
) -> Arc<BezPath> {
    let mut path = BezPath::new();
    path.move_to(origin);
    for index in 1..=400 {
        let fraction = index as f64 / 400.0;
        let response =
            curve.sample(Duration::from_secs_f64(fraction * seconds));
        path.line_to(
            origin + (fraction * WIDTH, response.position * HEIGHT),
        );
    }
    return Arc::new(path);
}

fn build_animation() -> CompiledScene {
    let experiments = experiments();
    let mut scene = Scene::new();
    scene.set_background(Color::from_rgb8(12, 15, 23));
    let label = TextStyle::new().font_size(13.0).fill(MUTED);
    let title = signal(Arc::<str>::from(experiments[0].name));
    let physical = signal(experiments[0].physical);
    let visual = signal(experiments[0].visual);
    let timed = signal(experiments[0].timed);
    let elapsed = signal(0.0);

    let physical_curve = computed!((physical) => physical.prepare().expect("valid physical settings"));
    let visual_curve = computed!((visual) => visual.prepare().expect("valid visual settings"));
    let timed_curve = computed!((timed) => timed.prepare().expect("valid timed settings"));
    let plot_seconds = computed!((physical_curve, visual_curve, timed_curve) => {
        return physical_curve.duration()
            .max(visual_curve.duration()).max(timed_curve.duration())
            .as_secs_f64().max(1.0);
    });

    scene.add(
        Text::new("SPRING LAB / PHYSICAL, VISUAL, TIMED")
            .position((-450.0, 450.0))
            .font_size(27.0)
            .fill(WHITE),
    );
    scene.add(
        Text::new(&title)
            .position((-450.0, 405.0))
            .font_size(18.0)
            .fill(WHITE),
    );

    let mut balls = Vec::new();
    for (name, color, y, settings, curve) in [
        (
            "PHYSICAL",
            Color::from_rgb8(90, 205, 235),
            150.0,
            &physical,
            &physical_curve,
        ),
        (
            "VISUAL",
            Color::from_rgb8(255, 190, 110),
            -100.0,
            &visual,
            &visual_curve,
        ),
        (
            "TIMED",
            Color::from_rgb8(193, 156, 255),
            -350.0,
            &timed,
            &timed_curve,
        ),
    ] {
        let origin = Point::new(-WIDTH / 2.0, y);
        scene.add(
            Text::new(name)
                .position(origin + (0.0, 150.0))
                .font_size(20.0)
                .fill(color),
        );
        scene.add(
            Text::new(computed!((settings, curve) => format!(
                "{settings} / SETTLES {:.2}S", curve.duration().as_secs_f64(),
            ).into()))
            .position(origin + (0.0, 120.0))
            .style(&label),
        );

        for tick in 0..=4 {
            let fraction = tick as f64 / 4.0;
            let x = origin.x + fraction * WIDTH;
            scene.add(
                Line::new((x, y), (x, y + HEIGHT * 2.0))
                    .stroke(GRID, 1.0),
            );
            scene.add(
                Text::new(computed!((plot_seconds) =>
                format!("{:.2}S", fraction * plot_seconds).into()))
                .position((x, y - 12.0))
                .style(&label)
                .align(TextAlign::Center),
            );
        }
        for response in [0.0, 0.5, 1.5, 2.0] {
            let y = y + response * HEIGHT;
            scene.add(
                Line::new((origin.x, y), (origin.x + WIDTH, y))
                    .stroke(GRID, 1.0),
            );
        }
        scene.add(
            Line::new(
                origin + (0.0, HEIGHT),
                origin + (WIDTH, HEIGHT),
            )
            .stroke(MUTED, 1.0)
            .line_dash([4.0, 6.0]),
        );
        scene.add(
            Path::from_bez_path(BezPath::new())
                .geometry(computed!((curve, plot_seconds) => graph(curve, plot_seconds, origin)))
                .stroke(color, 2.5),
        );
        // This marker follows the exact response the ball is currently sampling.
        scene.add(Circle::new(4.0).fill(WHITE).position(
            computed!((curve, plot_seconds, elapsed) => {
                let response = curve.sample(Duration::from_secs_f64(elapsed));
                return origin + (elapsed / plot_seconds * WIDTH, response.position * HEIGHT);
            }),
        ));

        scene.add(
            Line::new((-400.0, y - 65.0), (400.0, y - 65.0))
                .stroke(GRID, 1.0),
        );
        for x in [-TRAVEL, TRAVEL] {
            scene.add(
                Circle::new(12.0)
                    .position((x, y - 65.0))
                    .fill(TRANSPARENT)
                    .stroke(MUTED, 1.0),
            );
        }
        balls.push(
            scene.add(
                Circle::new(9.0)
                    .position((-TRAVEL, y - 65.0))
                    .fill(color),
            ),
        );
    }

    let mut steps = Vec::new();
    for (index, experiment) in experiments.into_iter().enumerate() {
        let springs = [
            experiment.physical,
            experiment.visual,
            experiment.timed,
        ];
        let duration = springs
            .iter()
            .map(|spring| {
                spring
                    .prepare()
                    .expect("valid experiment settings")
                    .duration()
            })
            .max()
            .expect("three springs");

        steps.push(title.set(experiment.name));
        // Signals already contain the first experiment's settings.
        if index > 0 {
            steps.push(all([
                physical.to(experiment.physical).over(ms(1000)),
                visual.to(experiment.visual).over(ms(1000)),
                timed.to(experiment.timed).over(ms(1000)),
            ]));
        }
        steps.push(wait(ms(500)));
        steps.push(all(balls
            .iter()
            .zip(springs)
            .map(|(ball, spring)| {
                ball.position_x().to(TRAVEL).spring(spring)
            })
            .chain([elapsed
                .to(duration.as_secs_f64())
                .over(duration)
                .ease(ease::linear)])));
        steps.push(wait(ms(100)));
        // Rewind along the current curves before morphing to new settings.
        let rewind = ms(500);
        steps.push(all(balls
            .iter()
            .map(|ball| ball.position_x().to(-TRAVEL))
            .chain([elapsed.to(0.0)])
            .map(|step| {
                step.over(rewind).ease(ease::cubic::ease_in_out)
            })));
    }
    scene.compile(steps)
}

fn main() -> Result<(), winit::error::EventLoopError> {
    Window::new("MotionGfx | Spring lab", CANVAS)
        .run(build_animation())
}

#[cfg(test)]
#[path = "support/spring_curves_tests.rs"]
mod tests;
