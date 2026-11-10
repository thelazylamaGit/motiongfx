//! Canvas Commons' insertion-sort example, recreated with the existing
//! MotionGfx API. No proposed authoring helpers or macros are used here.
//!
//! Run: cargo run -p vello_winit_example --example insertion_sort
//!
//! Each insertion pass reorders stable bar IDs, then moves ALL bars to
//! their new slots in parallel over 400ms. Even the already-sorted pass
//! takes 400ms, matching the original. The final picture holds for 600ms.
//! The algorithm constructs one timeline up front; it doesn't run again
//! during playback or scrubbing.

use std::time::Duration;

use motiongfx::prelude::*;
use vello::Scene;
use vello::kurbo::{Affine, Point, Rect, RoundedRect, Size};
use vello::peniko::{Color, Fill};
use vello_winit_example::{
    PlaybackConfig, PlaybackState, VelloDemo, VelloWinitApp, labels,
};
use winit::event_loop::EventLoop;

const VALUES: [u32; 6] = [5, 2, 8, 1, 6, 3];
const WIDTH: f64 = 900.0;
const HEIGHT: f64 = 600.0;
const PASS: Duration = ms(400);
const HOLD: Duration = ms(600);

fn slot(index: usize, count: usize) -> f64 {
    (index as f64 - count.saturating_sub(1) as f64 / 2.0) * 90.0
}

#[derive(Clone)]
struct Bar {
    value: u32,
    x: f64,
    // Fixed local geometry. Only x changes, so drawing can translate
    // this shape without rebuilding its corners each frame.
    shape: RoundedRect,
}

struct World {
    bars: Vec<Bar>,
}

impl World {
    fn new(values: &[u32]) -> Self {
        Self {
            bars: values
                .iter()
                .enumerate()
                .map(|(index, &value)| {
                    let height = f64::from(value) * 36.0;
                    Bar {
                        value,
                        x: slot(index, values.len()),
                        // Equivalent to width=64, height=value*36 and
                        // center y=200-height/2 in the original scene.
                        shape: RoundedRect::from_rect(
                            Rect::new(
                                -32.0,
                                200.0 - height,
                                32.0,
                                200.0,
                            ),
                            8.0,
                        ),
                    }
                })
                .collect(),
        }
    }
}

impl SubjectSource<usize, Bar> for World {
    fn get_source(&self, id: usize) -> Option<&Bar> {
        self.bars.get(id)
    }
    fn apply_source<R>(
        &mut self,
        id: usize,
        f: impl FnOnce(&mut Bar) -> R,
    ) -> Option<R> {
        self.bars.get_mut(id).map(f)
    }
}

fn build_timeline(world: &World) -> (Registry, Timeline<World>) {
    let mut registry = Registry::new();
    let mut b = registry.create_builder::<World>();
    // The world stays in insertion order. Only this list of (value, ID)
    // pairs is reordered, preserving the identity of every animated bar.
    let mut order: Vec<_> = world
        .bars
        .iter()
        .enumerate()
        .map(|(id, bar)| (bar.value, id))
        .collect();
    let mut passes = Vec::new();

    // Direct translation of the source's insertion-sort choreography.
    for i in 1..order.len() {
        let mut j = i;
        while j > 0 && order[j - 1].0 > order[j].0 {
            order.swap(j - 1, j);
            j -= 1;
        }

        let movement = order
            .iter()
            .enumerate()
            .map(|(index, &(_, id))| {
                let target = slot(index, order.len());
                b.act(id, path!(Bar.x), move |_| target)
                    .with_ease(ease::cubic::ease_in_out)
                    .play(PASS)
            })
            .ord_all();
        passes.push(movement);
    }

    passes.push(TrackFragment::silent(HOLD));
    let mut timeline = b.compile(passes.ord_chain().compile());
    timeline.bake_actions(&registry, world);
    (registry, timeline)
}

struct InsertionSort {
    world: World,
    registry: Registry,
    timeline: Timeline<World>,
    size: Size,
    fill: Color,
}

impl InsertionSort {
    fn new() -> Self {
        let world = World::new(&VALUES);
        let (registry, timeline) = build_timeline(&world);
        Self {
            world,
            registry,
            timeline,
            size: Size::new(WIDTH, HEIGHT),
            // The original --cc-blue fallback, #89b4fa.
            fill: Color::from_rgb8(137, 180, 250),
        }
    }
}

impl VelloDemo for InsertionSort {
    fn window_title(&self) -> &'static str {
        "MotionGfx | Insertion sort"
    }
    fn initial_logical_size(&self) -> (f64, f64) {
        (WIDTH, HEIGHT)
    }
    fn size_changed(&mut self, size: Size) {
        self.size = size;
    }
    fn playback_config(&self) -> PlaybackConfig {
        // Play once, matching the source. Use the shared bar to replay or
        // scrub backwards. The range is derived from the compiled track.
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

        // Canvas Commons uses centered coordinates. Preserve those while
        // fitting the entire illustration into the available viewport.
        let fit =
            (self.size.width / WIDTH).min(self.size.height / HEIGHT);
        let xf = Affine::scale(scale)
            * Affine::translate((
                (self.size.width - WIDTH * fit) / 2.0,
                (self.size.height - HEIGHT * fit) / 2.0,
            ))
            * Affine::scale(fit);
        let centered =
            xf * Affine::translate((WIDTH / 2.0, HEIGHT / 2.0));

        for bar in &self.world.bars {
            scene.fill(
                Fill::NonZero,
                centered * Affine::translate((bar.x, 0.0)),
                self.fill,
                None,
                &bar.shape,
            );
            // A small value annotation makes the sort easy to inspect.
            labels::draw(
                scene,
                centered,
                Point::new(bar.x - 6.0, 212.0),
                &bar.value.to_string(),
                Color::WHITE,
                2.0,
            );
        }

        labels::draw(
            scene,
            xf,
            Point::new(50.0, 40.0),
            "INSERTION SORT",
            self.fill,
            3.0,
        );
        labels::draw(
            scene,
            xf,
            Point::new(50.0, 76.0),
            "INPUT  5 2 8 1 6 3",
            Color::WHITE,
            2.0,
        );
        let pass = playback.time.as_millis() / PASS.as_millis();
        let status = if pass < (VALUES.len() - 1) as u128 {
            format!(
                "INSERTION PASS {} / {}",
                pass + 1,
                VALUES.len() - 1
            )
        } else {
            "SORTED / HOLD".to_owned()
        };
        labels::draw(
            scene,
            xf,
            Point::new(50.0, 110.0),
            &status,
            Color::WHITE,
            1.5,
        );
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloWinitApp::new(InsertionSort::new());
    event_loop.run_app(&mut app).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(
        world: &mut World,
        registry: &Registry,
        timeline: &mut Timeline<World>,
        time: Duration,
    ) {
        timeline.set_target_time(time);
        timeline.queue_actions();
        timeline.sample_queued_actions(registry, world);
    }

    fn assert_order(world: &World, expected_ids: &[usize]) {
        for (index, &id) in expected_ids.iter().enumerate() {
            assert!(
                (world.bars[id].x - slot(index, world.bars.len()))
                    .abs()
                    < 1e-4,
                "bar {id} is not in slot {index}"
            );
        }
    }

    #[test]
    fn matches_each_insertion_pass_and_source_duration() {
        let mut world = World::new(&VALUES);
        let (registry, mut timeline) = build_timeline(&world);
        assert_eq!(timeline.tracks()[0].duration(), ms(2600));
        assert_order(&world, &[0, 1, 2, 3, 4, 5]);
        for (pass, expected) in [
            [1, 0, 2, 3, 4, 5],
            [1, 0, 2, 3, 4, 5], // An unchanged pass still occupies 400ms.
            [3, 1, 0, 2, 4, 5],
            [3, 1, 0, 4, 2, 5],
            [3, 1, 5, 0, 4, 2],
        ]
        .iter()
        .enumerate()
        {
            sample(
                &mut world,
                &registry,
                &mut timeline,
                ms((pass as u64 + 1) * 400),
            );
            assert_order(&world, expected);
        }
        sample(&mut world, &registry, &mut timeline, ms(2600));
        assert_order(&world, &[3, 1, 5, 0, 4, 2]);
        // The final 600ms is a real hold, and seeking restores seeds.
        sample(&mut world, &registry, &mut timeline, ms(2100));
        assert_order(&world, &[3, 1, 5, 0, 4, 2]);
        sample(&mut world, &registry, &mut timeline, Duration::ZERO);
        assert_order(&world, &[0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn eased_midpoints_are_reproducible_when_seeking_backward() {
        let mut world = World::new(&VALUES);
        let (registry, mut timeline) = build_timeline(&world);
        sample(&mut world, &registry, &mut timeline, ms(200));
        assert!((world.bars[0].x + 180.0).abs() < 1e-4);
        assert!((world.bars[1].x + 180.0).abs() < 1e-4);
        for time in
            [ms(2600), ms(1400), ms(200), ms(1000), Duration::ZERO]
        {
            sample(&mut world, &registry, &mut timeline, time);
            let mut fresh = World::new(&VALUES);
            let (fresh_registry, mut fresh_timeline) =
                build_timeline(&fresh);
            sample(
                &mut fresh,
                &fresh_registry,
                &mut fresh_timeline,
                time,
            );
            for (a, b) in world.bars.iter().zip(&fresh.bars) {
                assert!((a.x - b.x).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn equal_values_preserve_their_original_id_order() {
        let mut world = World::new(&[2, 1, 2, 1]);
        let (registry, mut timeline) = build_timeline(&world);
        let end = timeline.tracks()[0].duration();
        sample(&mut world, &registry, &mut timeline, end);
        assert_order(&world, &[1, 3, 0, 2]);
    }
}
