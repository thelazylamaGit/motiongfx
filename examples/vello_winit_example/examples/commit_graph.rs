//! Canvas Commons commit graph: the same objects and choreography,
//! using current MotionGfx actions, PathTracer, and the shared Vello host.

use motiongfx::prelude::*;
use peniko_motiongfx::prelude::PathTracer;
use vello::Scene;
use vello::kurbo::{Affine, BezPath, Circle, Point, Size, Stroke};
use vello::peniko::{Color, Fill};
use vello_winit_example::{
    PlaybackConfig, PlaybackState, VelloDemo, VelloWinitApp,
};
use winit::event_loop::EventLoop;

#[derive(Clone)]
struct Commit {
    position: Point,
    fill: Color,
    scale: f64,
    pulse: f64,
}

impl Commit {
    fn new(x: f64, y: f64, fill: Color) -> Self {
        Self {
            position: Point::new(x, y),
            fill,
            scale: 0.0,
            pulse: 0.0,
        }
    }

    // Mirrors the custom Commit component: a filled circle with a child
    // ring whose opacity and scale depend on the parent's pulse field.
    fn draw(&self, scene: &mut Scene, parent: Affine) {
        if self.scale <= 0.0 {
            return;
        }
        let xf = parent
            * Affine::translate(self.position.to_vec2())
            * Affine::scale(self.scale);
        let circle = Circle::new((0.0, 0.0), 18.0);
        scene.fill(Fill::NonZero, xf, self.fill, None, &circle);
        scene.stroke(
            &Stroke::new(3.0),
            xf * Affine::scale(1.0 + self.pulse * 1.4),
            self.fill.with_alpha((1.0 - self.pulse) as f32),
            None,
            &circle,
        );
    }
}

struct World {
    lines: [PathTracer; 2],
    dots: Vec<Commit>,
}

impl World {
    fn new() -> Self {
        let main = Color::from_rgb8(137, 180, 250);
        let branch = Color::from_rgb8(203, 166, 247);
        let mut trunk = BezPath::new();
        trunk.move_to((-360.0, 60.0));
        trunk.line_to((200.0, 60.0));
        let mut fork = BezPath::new();
        fork.move_to((-120.0, 60.0));
        fork.line_to((40.0, -60.0));
        fork.line_to((240.0, -60.0));
        Self {
            lines: [trunk, fork].map(|path| PathTracer {
                path,
                t_start: 0.0,
                t_end: 0.0,
            }),
            dots: vec![
                Commit::new(-360.0, 60.0, main),
                Commit::new(-120.0, 60.0, main),
                Commit::new(200.0, 60.0, main),
                Commit::new(40.0, -60.0, branch),
                Commit::new(240.0, -60.0, branch),
            ],
        }
    }
}

impl SubjectSource<usize, PathTracer> for World {
    fn get_source(&self, id: usize) -> Option<&PathTracer> {
        self.lines.get(id)
    }
    fn apply_source<R>(
        &mut self,
        id: usize,
        f: impl FnOnce(&mut PathTracer) -> R,
    ) -> Option<R> {
        self.lines.get_mut(id).map(f)
    }
}

impl SubjectSource<usize, Commit> for World {
    fn get_source(&self, id: usize) -> Option<&Commit> {
        self.dots.get(id)
    }
    fn apply_source<R>(
        &mut self,
        id: usize,
        f: impl FnOnce(&mut Commit) -> R,
    ) -> Option<R> {
        self.dots.get_mut(id).map(f)
    }
}

fn build_timeline(world: &World) -> (Registry, Timeline<World>) {
    let mut registry = Registry::new();
    let mut b = registry.create_builder::<World>();
    let trunk = b
        .act(0usize, path!(PathTracer.t_end), |_| 1.0f32)
        .play(ms(800));
    let appear = (0..world.dots.len())
        .map(|id| {
            b.act(id, path!(Commit.scale), |_| 1.0)
                .with_ease(ease::back::ease_out)
                .play(ms(400))
        })
        .ord_flow(ms(120)); // sequence(0.12, ...)
    let fork = b
        .act(1usize, path!(PathTracer.t_end), |_| 1.0f32)
        .play(ms(800));
    let pulse = (0..world.dots.len())
        .map(|id| b.act(id, path!(Commit.pulse), |_| 1.0).play(s(1)))
        .ord_all();
    let track =
        [trunk, appear, fork, pulse, TrackFragment::silent(ms(400))]
            .ord_chain()
            .compile();
    let mut timeline = b.compile(track);
    timeline.bake_actions(&registry, world);
    (registry, timeline)
}

struct CommitGraph {
    world: World,
    registry: Registry,
    timeline: Timeline<World>,
    size: Size,
}

impl CommitGraph {
    fn new() -> Self {
        let world = World::new();
        let (registry, timeline) = build_timeline(&world);
        Self {
            world,
            registry,
            timeline,
            size: Size::new(960.0, 540.0),
        }
    }
}

impl VelloDemo for CommitGraph {
    fn window_title(&self) -> &'static str {
        "MotionGfx | Commit graph"
    }
    fn initial_logical_size(&self) -> (f64, f64) {
        (960.0, 540.0)
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
        let fit =
            (self.size.width / 960.0).min(self.size.height / 540.0);
        let xf = Affine::scale(scale)
            * Affine::translate((
                self.size.width / 2.0,
                self.size.height / 2.0,
            ))
            * Affine::scale(fit);
        for (line, color) in self.world.lines.iter().zip([
            Color::from_rgb8(137, 180, 250),
            Color::from_rgb8(203, 166, 247),
        ]) {
            scene.stroke(
                &Stroke::new(4.0),
                xf,
                color,
                None,
                &line.trace(),
            );
        }
        for dot in &self.world.dots {
            dot.draw(scene, xf);
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloWinitApp::new(CommitGraph::new());
    event_loop.run_app(&mut app).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample(demo: &mut CommitGraph, time: Duration) {
        demo.timeline.set_target_time(time);
        demo.timeline.queue_actions();
        demo.timeline
            .sample_queued_actions(&demo.registry, &mut demo.world);
    }

    #[test]
    fn matches_stagger_pulse_and_total_duration() {
        let mut demo = CommitGraph::new();
        assert_eq!(demo.timeline.tracks()[0].duration(), ms(3880));
        sample(&mut demo, ms(900));
        assert!(demo.world.dots[0].scale > 0.0);
        assert_eq!(demo.world.dots[1].scale, 0.0);
        sample(&mut demo, ms(1680));
        assert!(
            demo.world
                .dots
                .iter()
                .all(|d| (d.scale - 1.0).abs() < 1e-6)
        );
        assert_eq!(demo.world.lines[1].t_end, 0.0);
        sample(&mut demo, ms(2980));
        assert!(
            demo.world
                .dots
                .iter()
                .all(|d| (d.pulse - 0.5).abs() < 1e-6)
        );
        sample(&mut demo, ms(3880));
        assert!(demo.world.dots.iter().all(|d| d.pulse == 1.0));
        sample(&mut demo, Duration::ZERO);
        assert!(
            demo.world
                .dots
                .iter()
                .all(|d| d.scale == 0.0 && d.pulse == 0.0)
        );
        assert!(demo.world.lines.iter().all(|l| l.t_end == 0.0));
    }
}
