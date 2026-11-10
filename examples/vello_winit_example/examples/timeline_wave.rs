//! A MotionGfx-driven procedural wave, using the EXISTING builder API.
//!
//! Run: cargo run -p vello_winit_example --example timeline_wave
//!
//! MotionGfx controls amplitude, phase, frequency, and marker radius.
//! Geometry is calculated from those sampled values, never elapsed time.
//! The timeline grows the wave, moves it while changing its frequency,
//! holds it, reshapes it, moves it backward, and flattens it again.
//! Drag the shared playback bar to inspect any stage in either direction.

use std::f64::consts::TAU;
use std::time::Duration;

use motiongfx::prelude::*;
use vello::Scene;
use vello::kurbo::{Affine, BezPath, Circle, Point, Size, Stroke};
use vello::peniko::{Color, Fill};
use vello_winit_example::{
    PlaybackConfig, PlaybackMode, PlaybackState, VelloDemo,
    VelloWinitApp, labels,
};
use winit::event_loop::EventLoop;

const SEGMENTS: usize = 240;

/// Small, cloneable animation state. Large geometry buffers stay outside
/// this subject so MotionGfx's bake scratch doesn't need to clone them.
#[derive(Clone, Copy, Debug, PartialEq)]
struct WaveState {
    amplitude: f64, // Fraction of viewport height.
    phase: f64,     // Radians: increasing phase moves crests right.
    frequency: f64, // Radians across the width of the wave.
    marker_radius: f64,
}

impl Default for WaveState {
    fn default() -> Self {
        Self {
            amplitude: 0.0,
            phase: 0.0,
            frequency: 12.0,
            marker_radius: 6.0,
        }
    }
}

#[derive(Default)]
struct World {
    wave: WaveState,
}

// The one subject has ID 0. MotionGfx uses these methods to access it.
impl SubjectSource<usize, WaveState> for World {
    fn get_source(&self, id: usize) -> Option<&WaveState> {
        (id == 0).then_some(&self.wave)
    }

    fn apply_source<R>(
        &mut self,
        id: usize,
        f: impl FnOnce(&mut WaveState) -> R,
    ) -> Option<R> {
        (id == 0).then(|| f(&mut self.wave))
    }
}

fn build_timeline(world: &World) -> (Registry, Timeline<World>) {
    let mut registry = Registry::new();
    let mut b = registry.create_builder::<World>();

    let grow = b
        .act(0usize, path!(WaveState.amplitude), |_| 0.22)
        .with_ease(ease::cubic::ease_in_out)
        .play(s(1));

    // Different fields can animate simultaneously. Relative phase
    // actions are baked from the preceding action's endpoint.
    let move_and_tighten = [
        b.act(0usize, path!(WaveState.phase), |phase| {
            phase + TAU * 2.0
        })
        .play(s(4)),
        b.act(0usize, path!(WaveState.frequency), |_| 20.0)
            .with_ease(ease::cubic::ease_in_out)
            .play(s(4)),
        b.act(0usize, path!(WaveState.marker_radius), |_| 14.0)
            .with_ease(ease::cubic::ease_in_out)
            .play(s(4)),
    ]
    .ord_all();

    // With no active actions during this slot, the sampled values hold.
    // In contrast, sin(elapsed_time) would keep moving through the pause.
    let hold = TrackFragment::silent(s(1));

    let reshape = [
        b.act(0usize, path!(WaveState.amplitude), |_| 0.12)
            .with_ease(ease::cubic::ease_in_out)
            .play(s(2)),
        b.act(0usize, path!(WaveState.frequency), |_| 6.0)
            .with_ease(ease::cubic::ease_in_out)
            .play(s(2)),
        b.act(0usize, path!(WaveState.marker_radius), |_| 8.0)
            .with_ease(ease::cubic::ease_in_out)
            .play(s(2)),
    ]
    .ord_all();

    let move_back = b
        .act(0usize, path!(WaveState.phase), |phase| phase - TAU)
        .play(s(3));
    let flatten = b
        .act(0usize, path!(WaveState.amplitude), |_| 0.0)
        .with_ease(ease::cubic::ease_in_out)
        .play(s(1));

    let track = [
        grow,
        move_and_tighten,
        hold,
        reshape,
        move_back,
        flatten,
        TrackFragment::silent(s(1)),
    ]
    .ord_chain()
    .compile();
    let mut timeline = b.compile(track);
    timeline.bake_actions(&registry, world);
    (registry, timeline)
}

/// All procedural math is here. Time only enters through MotionGfx's
/// sampled fields, so holds, reverse playback, and random seeks agree.
fn wave_point(state: &WaveState, size: Size, fraction: f64) -> Point {
    let x = 40.0 + fraction * (size.width - 80.0).max(0.0);
    let angle = fraction * state.frequency - state.phase;
    let y = size.height * (0.5 + state.amplitude * angle.sin());
    Point::new(x, y)
}

struct TimelineWave {
    world: World,
    registry: Registry,
    timeline: Timeline<World>,
    size: Size,
    path: BezPath,
    marker: Circle,
    stroke: Stroke,
    color: Color,
    last_sample: Option<(Duration, Size)>,
}

impl TimelineWave {
    fn new() -> Self {
        let world = World::default();
        let (registry, timeline) = build_timeline(&world);
        Self {
            world,
            registry,
            timeline,
            size: Size::new(900.0, 420.0),
            // Reserve once. Rebuilding geometry retains this allocation.
            path: BezPath::with_capacity(SEGMENTS + 1),
            marker: Circle::new((0.0, 0.0), 6.0),
            stroke: Stroke::new(3.0),
            color: Color::from_rgb8(72, 222, 194),
            last_sample: None,
        }
    }
}

impl VelloDemo for TimelineWave {
    fn window_title(&self) -> &'static str {
        "MotionGfx | Timeline-driven wave"
    }
    fn initial_logical_size(&self) -> (f64, f64) {
        (900.0, 420.0)
    }
    fn size_changed(&mut self, size: Size) {
        self.size = size;
    }

    fn playback_config(&self) -> PlaybackConfig {
        // The preview range comes from the actual animation, not a second
        // hand-maintained duration. Editing an action updates this too.
        PlaybackConfig::new(self.timeline.tracks()[0].duration())
            .with_mode(PlaybackMode::PingPong)
    }

    fn rebuild_scene(
        &mut self,
        scene: &mut Scene,
        scale: f64,
        playback: PlaybackState,
    ) {
        if self.last_sample != Some((playback.time, self.size)) {
            self.timeline.set_target_time(playback.time);
            self.timeline.queue_actions();
            self.timeline.sample_queued_actions(
                &self.registry,
                &mut self.world,
            );

            self.path.truncate(0); // Clear elements, retain capacity.
            self.path.move_to(wave_point(
                &self.world.wave,
                self.size,
                0.0,
            ));
            for i in 1..=SEGMENTS {
                self.path.line_to(wave_point(
                    &self.world.wave,
                    self.size,
                    i as f64 / SEGMENTS as f64,
                ));
            }
            self.marker.center =
                wave_point(&self.world.wave, self.size, 0.5);
            self.marker.radius = self.world.wave.marker_radius;
            self.last_sample = Some((playback.time, self.size));
        }

        let xf = Affine::scale(scale);
        scene.stroke(&self.stroke, xf, self.color, None, &self.path);
        scene.fill(
            Fill::NonZero,
            xf,
            Color::WHITE,
            None,
            &self.marker,
        );

        // Text describes the timeline; it doesn't drive the animation.
        let seconds = playback.time.as_secs_f64();
        let chapter = if seconds < 1.0 {
            "GROW AMPLITUDE"
        } else if seconds < 5.0 {
            "MOVE PHASE / TIGHTEN FREQUENCY / GROW MARKER"
        } else if seconds < 6.0 {
            "HOLD - GEOMETRY STAYS STILL"
        } else if seconds < 8.0 {
            "RESHAPE AMPLITUDE AND FREQUENCY"
        } else if seconds < 11.0 {
            "MOVE PHASE BACKWARD"
        } else if seconds < 12.0 {
            "FLATTEN AMPLITUDE"
        } else {
            "HOLD - THEN REVERSE"
        };
        labels::draw(
            scene,
            xf,
            Point::new(40.0, 22.0),
            chapter,
            self.color,
            2.0,
        );
        let state = &self.world.wave;
        labels::draw(
            scene,
            xf,
            Point::new(40.0, 52.0),
            &format!(
                "AMPLITUDE {:.2}  PHASE {:.2}  FREQUENCY {:.2}",
                state.amplitude, state.phase, state.frequency
            ),
            Color::WHITE,
            1.5,
        );
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloWinitApp::new(TimelineWave::new());
    event_loop.run_app(&mut app).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(demo: &mut TimelineWave, seconds: f64) -> WaveState {
        demo.timeline
            .set_target_time(Duration::from_secs_f64(seconds));
        demo.timeline.queue_actions();
        demo.timeline
            .sample_queued_actions(&demo.registry, &mut demo.world);
        demo.world.wave
    }

    fn assert_close(a: WaveState, b: WaveState) {
        for (a, b) in [
            (a.amplitude, b.amplitude),
            (a.phase, b.phase),
            (a.frequency, b.frequency),
            (a.marker_radius, b.marker_radius),
        ] {
            assert!((a - b).abs() < 1e-6, "{a} != {b}");
        }
    }

    #[test]
    fn silent_slot_really_holds_the_wave() {
        let mut demo = TimelineWave::new();
        let first = sample(&mut demo, 5.1);
        let second = sample(&mut demo, 5.9);
        assert_close(first, second);
        assert_eq!(
            wave_point(&first, demo.size, 0.3),
            wave_point(&second, demo.size, 0.3)
        );
        assert!((first.phase - TAU * 2.0).abs() < 1e-6);
    }

    #[test]
    fn seeking_backward_matches_sampling_fresh() {
        let mut demo = TimelineWave::new();
        assert_eq!(demo.timeline.tracks()[0].duration(), s(13));
        let end = sample(&mut demo, 13.0);
        assert!(end.amplitude.abs() < 1e-6);
        assert!((end.phase - TAU).abs() < 1e-6);
        for seconds in [9.0, 3.0, 7.0, 0.5, 5.5, 0.0] {
            let mut fresh = TimelineWave::new();
            assert_close(
                sample(&mut demo, seconds),
                sample(&mut fresh, seconds),
            );
        }
        assert_close(demo.world.wave, WaveState::default());
    }
}
