//! Manim's SineCurveUnitCircle, using an existing MotionGfx PathTracer.
//! The full sine path is generated ONCE. Its animated visible prefix also
//! determines the orbiting dot and both connecting lines. No dt history
//! or new reconstruction framework is needed to seek backward.

use std::f64::consts::TAU;

use motiongfx::prelude::*;
use peniko_motiongfx::prelude::PathTracer;
use vello::Scene;
use vello::kurbo::{
    Affine, BezPath, Circle, Line, Point, Size, Stroke,
};
use vello::peniko::{Color, Fill};
use vello_winit_example::{
    PlaybackConfig, PlaybackState, VelloDemo, VelloWinitApp, labels,
};
use winit::event_loop::EventLoop;

const TURNS: f64 = 8.5 * 0.25;
const SEGMENTS: usize = 1088;
const ORIGIN: Point = Point::new(-4.0, 0.0);

fn orbit_point(turns: f64) -> Point {
    Point::new(-4.0 + (turns * TAU).cos(), (turns * TAU).sin())
}

fn curve_point(turns: f64) -> Point {
    Point::new(-3.0 + turns * 4.0, (turns * TAU).sin())
}

struct World {
    curve: PathTracer,
}

impl World {
    fn new() -> Self {
        let mut path = BezPath::with_capacity(SEGMENTS + 1);
        path.move_to(curve_point(0.0));
        for i in 1..=SEGMENTS {
            path.line_to(curve_point(
                TURNS * i as f64 / SEGMENTS as f64,
            ));
        }
        Self {
            curve: PathTracer {
                path,
                t_start: 0.0,
                t_end: 0.0,
            },
        }
    }
}

impl SubjectSource<usize, PathTracer> for World {
    fn get_source(&self, id: usize) -> Option<&PathTracer> {
        (id == 0).then_some(&self.curve)
    }
    fn apply_source<R>(
        &mut self,
        id: usize,
        f: impl FnOnce(&mut PathTracer) -> R,
    ) -> Option<R> {
        (id == 0).then(|| f(&mut self.curve))
    }
}

fn build_timeline(world: &World) -> (Registry, Timeline<World>) {
    let mut registry = Registry::new();
    let mut b = registry.create_builder::<World>();
    let draw = b
        .act(0usize, path!(PathTracer.t_end), |_| 1.0f32)
        .play(ms(8500));
    // The source removes the updater after 8.5s, then self.wait() holds 1s.
    let mut timeline = b.compile(
        [draw, TrackFragment::silent(s(1))].ord_chain().compile(),
    );
    timeline.bake_actions(&registry, world);
    (registry, timeline)
}

struct SineUnitCircle {
    world: World,
    registry: Registry,
    timeline: Timeline<World>,
    size: Size,
    visible: BezPath,
    last_progress: Option<f32>,
}

impl SineUnitCircle {
    fn new() -> Self {
        let world = World::new();
        let (registry, timeline) = build_timeline(&world);
        Self {
            world,
            registry,
            timeline,
            size: Size::new(1120.0, 600.0),
            visible: BezPath::new(),
            last_progress: None,
        }
    }
}

impl VelloDemo for SineUnitCircle {
    fn window_title(&self) -> &'static str {
        "MotionGfx | Unit circle and sine curve"
    }
    fn initial_logical_size(&self) -> (f64, f64) {
        (1120.0, 600.0)
    }
    fn size_changed(&mut self, size: Size) {
        self.size = size;
    }
    fn playback_config(&self) -> PlaybackConfig {
        PlaybackConfig::new(self.timeline.tracks()[0].duration())
    }
    fn rebuild_scene(
        &mut self,
        scene: &mut Scene,
        scale: f64,
        playback: PlaybackState,
    ) {
        self.timeline.set_target_time(playback.time);
        self.timeline.queue_actions();
        self.timeline
            .sample_queued_actions(&self.registry, &mut self.world);
        let progress = self.world.curve.t_end;
        if self.last_progress != Some(progress) {
            self.visible = self.world.curve.trace();
            self.last_progress = Some(progress);
        }
        let fit =
            (self.size.width / 1120.0).min(self.size.height / 600.0);
        let screen = Affine::scale(scale)
            * Affine::translate((
                self.size.width / 2.0,
                self.size.height / 2.0,
            ))
            * Affine::scale(fit);
        let math = screen * Affine::scale_non_uniform(80.0, -80.0);
        let white = Color::WHITE;
        let blue = Color::from_rgb8(88, 196, 221);
        let yellow = Color::from_rgb8(247, 217, 111);
        for axis in [
            Line::new((-6.0, 0.0), (6.0, 0.0)),
            Line::new((-4.0, -2.0), (-4.0, 2.0)),
        ] {
            scene.stroke(
                &Stroke::new(2.0 / 80.0),
                math,
                white,
                None,
                &axis,
            );
        }
        scene.stroke(
            &Stroke::new(4.0 / 80.0),
            math,
            white,
            None,
            &Circle::new(ORIGIN, 1.0),
        );

        let turns = f64::from(progress) * TURNS;
        let dot = orbit_point(turns);
        let tip = curve_point(turns);
        scene.stroke(
            &Stroke::new(4.0 / 80.0),
            math,
            blue,
            None,
            &Line::new(ORIGIN, dot),
        );
        scene.stroke(
            &Stroke::new(2.0 / 80.0),
            math,
            Color::from_rgb8(255, 241, 182),
            None,
            &Line::new(dot, tip),
        );
        scene.stroke(
            &Stroke::new(4.0 / 80.0),
            math,
            Color::from_rgb8(244, 211, 69),
            None,
            &self.visible,
        );
        scene.fill(
            Fill::NonZero,
            math,
            yellow,
            None,
            &Circle::new(dot, 0.08),
        );

        // ASCII labels stand in for MathTex; no typesetting integration
        // is added just to reproduce these four labels.
        for (index, text) in
            ["PI", "2 PI", "3 PI", "4 PI"].iter().enumerate()
        {
            let x = (-1.0 + index as f64 * 2.0) * 80.0;
            labels::draw(
                scene,
                screen,
                Point::new(x - 12.0, 18.0),
                text,
                white,
                2.0,
            );
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloWinitApp::new(SineUnitCircle::new());
    event_loop.run_app(&mut app).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample(demo: &mut SineUnitCircle, time: Duration) {
        demo.timeline.set_target_time(time);
        demo.timeline.queue_actions();
        demo.timeline
            .sample_queued_actions(&demo.registry, &mut demo.world);
    }

    #[test]
    fn trace_tracks_orbit_and_can_rewind() {
        let mut demo = SineUnitCircle::new();
        assert_eq!(demo.timeline.tracks()[0].duration(), ms(9500));
        sample(&mut demo, s(2));
        let turns = f64::from(demo.world.curve.t_end) * TURNS;
        assert!((turns - 0.5).abs() < 1e-6);
        assert!(
            orbit_point(turns).distance(Point::new(-5.0, 0.0)) < 1e-6
        );
        let path = demo.world.curve.trace();
        let end =
            path.elements().last().unwrap().end_point().unwrap();
        assert!(end.distance(curve_point(turns)) < 1e-3);
        sample(&mut demo, ms(9500));
        assert_eq!(demo.world.curve.t_end, 1.0);
        sample(&mut demo, ms(8500));
        assert_eq!(demo.world.curve.t_end, 1.0);
        sample(&mut demo, Duration::ZERO);
        assert_eq!(demo.world.curve.t_end, 0.0);
        assert!(demo.world.curve.trace().is_empty());
    }
}
