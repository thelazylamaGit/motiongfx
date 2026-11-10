//! A small, annotated lesson in quadratic Bezier curves and MotionGfx.
//!
//! Run: `cargo run -p vello_winit_example --example bezier_explainer`
//!
//! Read this file in order: math -> world -> timeline -> drawing -> app.
//! The animation draws a grid, traces a curve using de Casteljau's
//! construction, then bends the curve by moving its middle control point.
//! It plays forward and backward automatically to demonstrate seeking.
//! Drag the bottom timeline to scrub; Space resumes/pauses playback.
//! Left/Right seek by 0.25s; Home/End jump to the start/end.
//!
//! MotionGfx animates just five numbers. Ordinary Rust derives all the
//! geometry from those numbers each frame; Vello draws that geometry.
//! No scripting or scene serialization is needed for this example.

use std::time::Duration;

use motiongfx::prelude::*;
use vello::Scene;
use vello::kurbo::{
    Affine, BezPath, Circle, Line, Point, Rect, Size, Stroke,
};
use vello::peniko::{Color, Fill};
use vello_winit_example::{
    PlaybackConfig, PlaybackMode, PlaybackState, VelloDemo,
    VelloWinitApp,
};
use winit::event_loop::EventLoop;

// The host shares a tiny vector label helper with the demos, so font
// setup doesn't obscure the animation lesson.
use vello_winit_example::labels;

// All drawing uses this fixed logical canvas, fitted into the window.
const WIDTH: f64 = 1120.0;
const HEIGHT: f64 = 800.0;
const INK: [u8; 3] = [230, 235, 246];
const MUTED: [u8; 3] = [146, 157, 184];
const TEAL: [u8; 3] = [72, 222, 194];
const GOLD: [u8; 3] = [255, 197, 103];
const PINK: [u8; 3] = [248, 128, 170];

// 1. THE MATH ---------------------------------------------------------

/// Linear interpolation: t=0 gives a, t=1 gives b.
/// kurbo represents positions as Points and differences as Vec2s.
fn lerp(a: Point, b: Point, t: f64) -> Point {
    a + (b - a) * t
}

/// De Casteljau's algorithm uses the SAME t for three interpolations:
/// A = lerp(P0, P1, t), B = lerp(P1, P2, t), Q = lerp(A, B, t).
/// Q lies on the quadratic Bezier curve; A and B explain why.
fn construction(points: [Point; 3], t: f64) -> [Point; 3] {
    let [p0, p1, p2] = points;
    let a = lerp(p0, p1, t);
    let b = lerp(p1, p2, t);
    [a, b, lerp(a, b, t)]
}

// 2. THE WORLD --------------------------------------------------------

/// This is our one animatable subject. Clone is needed because baking
/// works on a scratch copy rather than changing the visible world.
#[derive(Clone, Default)]
struct Diagram {
    grid: f64,   // 0..1: how much of each grid line has drawn in.
    guides: f64, // 0..1: opacity of the control polygon/construction.
    reveal: f64, // 0..1: visible parameter interval of the curve.
    t: f64,      // 0..1: the probe's position along that curve.
    bend: f64,   // Vertical offset of P1, measured in logical pixels.
}

impl Diagram {
    fn points(&self) -> [Point; 3] {
        [
            Point::new(160.0, 550.0),
            // Screen y increases downward: positive bend lowers P1.
            Point::new(560.0, 230.0 + self.bend),
            Point::new(960.0, 550.0),
        ]
    }
}

/// MotionGfx doesn't require an ECS. A world can be a simple struct.
#[derive(Default)]
struct World {
    diagram: Diagram,
}

/// Connect subject ID 0 to our Diagram. Returning None for other IDs
/// prevents accidentally addressing the wrong subject.
impl SubjectSource<usize, Diagram> for World {
    fn get_source(&self, id: usize) -> Option<&Diagram> {
        (id == 0).then_some(&self.diagram)
    }

    fn apply_source<R>(
        &mut self,
        id: usize,
        f: impl FnOnce(&mut Diagram) -> R,
    ) -> Option<R> {
        (id == 0).then(|| f(&mut self.diagram))
    }
}

// 3. THE TIMELINE -----------------------------------------------------

/// Construct and bake once. There is no per-frame animation allocation.
fn build_timeline(world: &World) -> (Registry, Timeline<World>) {
    // Registry stores the typed access and sampling machinery.
    let mut registry = Registry::new();
    let mut b = registry.create_builder::<World>();

    // act(subject, field path, target function) describes an action.
    // |_| ignores the previous value and supplies an absolute target.
    // path! creates a typed lens, not a string lookup.
    // play(duration) assigns timing; it doesn't start a running task.
    let grid = b
        .act(0usize, path!(Diagram.grid), |_| 1.0)
        .with_ease(ease::cubic::ease_in_out)
        .play(ms(900));

    let guides =
        b.act(0usize, path!(Diagram.guides), |_| 1.0).play(ms(600));

    // These are separate fields, animated simultaneously by ord_all.
    // No easing here: t should advance uniformly for the math lesson.
    let probe = b.act(0usize, path!(Diagram.t), |_| 1.0).play(s(4));
    let reveal =
        b.act(0usize, path!(Diagram.reveal), |_| 1.0).play(s(4));
    let trace = [probe, reveal].ord_all();

    // The next action on t starts from the previous action's endpoint.
    let center = b
        .act(0usize, path!(Diagram.t), |_| 0.5)
        .with_ease(ease::cubic::ease_in_out)
        .play(s(1));

    // Reusable procedural authoring: a loop creates three relative
    // actions. Their endpoint functions run at BAKE time, not per frame.
    // Starting at 0, bend becomes 200, then -40, then 0 again.
    let bends = [200.0, -240.0, 40.0]
        .into_iter()
        .map(|offset| {
            b.act(0usize, path!(Diagram.bend), move |previous| {
                previous + offset
            })
            .with_ease(ease::cubic::ease_in_out)
            .play(ms(1800))
        })
        .ord_chain();

    let finish = b
        .act(0usize, path!(Diagram.t), |_| 1.0)
        .with_ease(ease::cubic::ease_in_out)
        .play(ms(1500));

    // ord_chain lays fragments end to end. A silent fragment adds a
    // reading pause without inventing an action on a dummy field.
    let track = [
        grid,
        guides,
        trace,
        center,
        bends,
        finish,
        TrackFragment::silent(s(1)),
    ]
    .ord_chain()
    .compile();

    // Compile resolves scheduling; bake computes start/end values from
    // the initial world. Baking takes &World and doesn't mutate it.
    let mut timeline = b.compile(track);
    timeline.bake_actions(&registry, world);
    (registry, timeline)
}

/// One entry point for both real-time playback and the headless tests.
fn sample(
    timeline: &mut Timeline<World>,
    registry: &Registry,
    world: &mut World,
    time: Duration,
) {
    timeline.set_target_time(time); // Absolute time, not a delta.
    timeline.queue_actions(); // Find the fields that need sampling.
    timeline.sample_queued_actions(registry, world); // Write values.
}

// 4. THE DRAWING ------------------------------------------------------

/// Centralize opacity conversion so all geometry uses the same palette.
fn color(rgb: [u8; 3], opacity: f64) -> Color {
    Color::from_rgba8(
        rgb[0],
        rgb[1],
        rgb[2],
        (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

fn line(
    scene: &mut Scene,
    xf: Affine,
    a: Point,
    b: Point,
    ink: Color,
    width: f64,
) {
    scene.stroke(
        &Stroke::new(width),
        xf,
        ink,
        None,
        &Line::new(a, b),
    );
}

fn dot(
    scene: &mut Scene,
    xf: Affine,
    p: Point,
    ink: Color,
    radius: f64,
) {
    scene.fill(Fill::NonZero, xf, ink, None, &Circle::new(p, radius));
}

fn label(
    scene: &mut Scene,
    xf: Affine,
    x: f64,
    y: f64,
    text: &str,
    ink: Color,
    size: f64,
) {
    labels::draw(scene, xf, Point::new(x, y), text, ink, size);
}

/// Drawing is a pure interpretation of the sampled world: no += dt,
/// accumulated trails, or hidden simulation state. Backward seeking
/// therefore reconstructs the same picture as forward playback.
fn draw_diagram(scene: &mut Scene, xf: Affine, d: &Diagram) {
    // A subtle panel gives the graph its own coordinate space.
    scene.fill(
        Fill::NonZero,
        xf,
        color([22, 27, 43], 1.0),
        None,
        &Rect::new(80.0, 170.0, 1040.0, 610.0),
    );

    // Generate grid geometry with ordinary Rust loops. A single animated
    // scalar controls the endpoints of every line.
    for column in 0..=24 {
        let x = 80.0 + column as f64 * 40.0;
        line(
            scene,
            xf,
            Point::new(x, 170.0),
            Point::new(x, 170.0 + 440.0 * d.grid),
            color(MUTED, 0.13),
            1.0,
        );
    }
    for row in 0..=11 {
        let y = 170.0 + row as f64 * 40.0;
        line(
            scene,
            xf,
            Point::new(80.0, y),
            Point::new(80.0 + 960.0 * d.grid, y),
            color(MUTED, 0.13),
            1.0,
        );
    }

    let [p0, p1, p2] = d.points();
    let [a, b, q] = construction(d.points(), d.t);

    // The original triangle is the control polygon, not the curve.
    for (start, end) in [(p0, p1), (p1, p2)] {
        line(
            scene,
            xf,
            start,
            end,
            color(MUTED, d.guides * 0.6),
            1.5,
        );
    }

    // Build a polyline over [0, reveal]. 180 samples are plenty for this
    // diagram; the marker itself uses the exact formula, not this mesh.
    if d.reveal > 0.0 {
        let mut curve = BezPath::new();
        curve.move_to(p0);
        for i in 1..=180 {
            let t = d.reveal * i as f64 / 180.0;
            curve.line_to(construction(d.points(), t)[2]);
        }
        scene.stroke(
            &Stroke::new(4.0),
            xf,
            color(TEAL, 1.0),
            None,
            &curve,
        );
    }

    // This moving gold segment is the second interpolation. Its point
    // Q sweeps out the curve even though neither A nor B follows a curve.
    line(scene, xf, a, b, color(GOLD, d.guides), 2.0);
    for p in [a, b] {
        dot(scene, xf, p, color(GOLD, d.guides), 5.0);
    }
    for (p, name) in [(p0, "P0"), (p1, "P1"), (p2, "P2")] {
        dot(scene, xf, p, color(INK, d.guides), 6.0);
        label(
            scene,
            xf,
            p.x + 12.0,
            p.y - 25.0,
            name,
            color(INK, d.guides),
            2.0,
        );
    }
    // Keep the moving labels visible even at t=0 and t=1. Text can
    // overlap freely; place B below its point to separate it from P2.
    label(
        scene,
        xf,
        a.x - 22.0,
        a.y - 25.0,
        "A",
        color(GOLD, d.guides),
        2.0,
    );
    label(
        scene,
        xf,
        b.x + 12.0,
        b.y + 14.0,
        "B",
        color(GOLD, d.guides),
        2.0,
    );
    // A broad, translucent dot gives Q a small halo.
    dot(scene, xf, q, color(PINK, d.guides * 0.16), 16.0);
    dot(scene, xf, q, color(PINK, d.guides), 7.0);
}

// 5. THE APP ----------------------------------------------------------

/// Fit the lesson into the drawable area; the host owns the control strip.
fn canvas_layout(window: Size) -> (f64, (f64, f64)) {
    let fit = (window.width / WIDTH).min(window.height / HEIGHT);
    (
        fit,
        (
            (window.width - WIDTH * fit) / 2.0,
            (window.height - HEIGHT * fit) / 2.0,
        ),
    )
}

struct BezierDemo {
    world: World,
    registry: Registry,
    timeline: Timeline<World>,
    window_size: Size,
}

impl BezierDemo {
    fn new() -> Self {
        let world = World::default();
        let (registry, timeline) = build_timeline(&world);
        Self {
            world,
            registry,
            timeline,
            window_size: Size::new(WIDTH, HEIGHT),
        }
    }
}

impl VelloDemo for BezierDemo {
    fn window_title(&self) -> &'static str {
        "MotionGfx | A curve from three points"
    }

    fn initial_logical_size(&self) -> (f64, f64) {
        (WIDTH, HEIGHT)
    }

    fn size_changed(&mut self, size: Size) {
        self.window_size = size;
    }

    fn playback_config(&self) -> PlaybackConfig {
        PlaybackConfig::new(self.timeline.tracks()[0].duration())
            .with_mode(PlaybackMode::PingPong)
    }

    fn rebuild_scene(
        &mut self,
        scene: &mut Scene,
        scale_factor: f64,
        playback: PlaybackState,
    ) {
        let reversing = playback.reversing;
        let time = playback.time.as_secs_f64();
        sample(
            &mut self.timeline,
            &self.registry,
            &mut self.world,
            playback.time,
        );

        // Fit the logical canvas without distorting circles. Include
        // the OS scale factor to make coordinates correct on HiDPI.
        let (fit, offset) = canvas_layout(self.window_size);
        let xf = Affine::scale(scale_factor)
            * Affine::translate(offset)
            * Affine::scale(fit);

        label(
            scene,
            xf,
            80.0,
            42.0,
            "A CURVE FROM THREE POINTS",
            color(INK, 1.0),
            4.0,
        );
        label(
            scene,
            xf,
            80.0,
            96.0,
            "QUADRATIC BEZIER / DE CASTELJAU CONSTRUCTION",
            color(MUTED, 1.0),
            2.0,
        );
        let chapter = if time < 1.5 {
            "01  BUILD THE GRID AND CONTROL POLYGON"
        } else if time < 6.5 {
            "02  SLIDE TWO POINTS - TRACE A CURVE"
        } else if time < 11.9 {
            "03  MOVE P1 - THE WHOLE CURVE RESPONDS"
        } else {
            "04  SAME MATH AT EVERY TIMELINE POSITION"
        };
        label(scene, xf, 80.0, 134.0, chapter, color(TEAL, 1.0), 2.0);
        draw_diagram(scene, xf, &self.world.diagram);

        label(
            scene,
            xf,
            80.0,
            644.0,
            "A = LERP(P0, P1, T)",
            color(GOLD, 1.0),
            2.0,
        );
        label(
            scene,
            xf,
            80.0,
            672.0,
            "B = LERP(P1, P2, T)",
            color(GOLD, 1.0),
            2.0,
        );
        label(
            scene,
            xf,
            80.0,
            700.0,
            "Q = LERP(A, B, T)",
            color(PINK, 1.0),
            2.0,
        );
        label(
            scene,
            xf,
            660.0,
            644.0,
            &format!(
                "T = {:.2}  TIME {:.2}S",
                self.world.diagram.t, time
            ),
            color(INK, 1.0),
            2.0,
        );
        label(
            scene,
            xf,
            660.0,
            686.0,
            if !playback.playing {
                "PAUSED / SPACE TO PLAY"
            } else if reversing {
                "REVERSE / SAME TIMELINE"
            } else {
                "FORWARD / AUTO REVERSE"
            },
            color(MUTED, 1.0),
            2.0,
        );
    }
}

fn main() {
    // The shared harness owns Winit, the surface, GPU, and Vello renderer.
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = VelloWinitApp::new(BezierDemo::new());
    event_loop.run_app(&mut app).expect("run example");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_matches_the_quadratic_equation() {
        let points = Diagram::default().points();
        // Compare the geometric construction with the independent
        // Bernstein-polynomial definition, including both endpoints.
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let u = 1.0 - t;
            let expected = points[0].to_vec2() * (u * u)
                + points[1].to_vec2() * (2.0 * u * t)
                + points[2].to_vec2() * (t * t);
            assert!(
                (construction(points, t)[2].to_vec2() - expected)
                    .hypot()
                    < 1e-9
            );
        }
    }

    #[test]
    fn timeline_can_seek_back_after_relative_bends() {
        let mut world = World::default();
        let (registry, mut timeline) = build_timeline(&world);
        // The peak of the first relative bend is at 6.5 + 1.8 seconds.
        sample(&mut timeline, &registry, &mut world, ms(8300));
        assert!((world.diagram.bend - 200.0).abs() < 1e-6);
        let expected =
            construction(world.diagram.points(), world.diagram.t)[2];

        let end = timeline.tracks()[0].duration();
        sample(&mut timeline, &registry, &mut world, end);
        assert!(world.diagram.bend.abs() < 1e-6);
        sample(&mut timeline, &registry, &mut world, ms(8300));
        assert!(
            (construction(world.diagram.points(), world.diagram.t)
                [2]
                - expected)
                .hypot()
                < 1e-6
        );

        sample(&mut timeline, &registry, &mut world, Duration::ZERO);
        let d = &world.diagram;
        for value in [d.grid, d.guides, d.reveal, d.t, d.bend] {
            assert!(
                value.abs() < 1e-6,
                "field did not reset: {value}"
            );
        }
    }
}
