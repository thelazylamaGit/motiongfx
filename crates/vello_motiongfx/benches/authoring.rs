//! Authoring, sampling and encoding benchmark for the 2D stack.
//!
//! Records a baseline before the `Animation` redesign and compares after it
//! (see docs/authoring-redesign.md, "Measurements"). Only [`animate`] is
//! ported to the new API; object setup, the authored timeline and the
//! benchmark IDs stay identical so the comparison is fair.
//!
//! Phases, measured separately:
//!   * `objects`: create the circles (scene construction)
//!   * `animate`: author, lower, compile and bake the timeline
//!   * `sample_forward`: a forward playback sweep (the per-frame path)
//!   * `seek_backward`: the same sweep backwards
//!   * `encode`: the renderer recording one frame into a `vello::Scene`
//!
//! Run:
//!   cargo bench -p vello_motiongfx --bench authoring -- --save-baseline before
//!   cargo bench -p vello_motiongfx --bench authoring -- --baseline before

use std::hint::black_box;
use std::time::Duration;

use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, criterion_group,
    criterion_main,
};
use motiongfx::prelude::{ease, ms};
use vello_motiongfx::{
    Circle, CircleHandle, Color, CompiledScene, Renderer, Scene,
    Signal, Size, Spring, all, flow, signal, wait,
};

const SIZES: [usize; 2] = [1_000, 10_000];
/// Samples per sweep; throughput is reported per frame.
const FRAMES: u32 = 120;
const ORANGE: Color = Color::from_rgb8(255, 134, 47);

struct Objects {
    scene: Scene,
    circles: Vec<CircleHandle>,
    progress: Signal<f64>,
}

/// A grid of circles. Every tenth radius follows a shared signal, so
/// sampling also evaluates bindings.
fn objects(count: usize) -> Objects {
    let mut scene = Scene::new();
    let progress = signal(0.0);
    let columns = (count as f64).sqrt().ceil() as usize;
    let circles = (0..count)
        .map(|index| {
            let position = (
                (index % columns) as f64 * 8.0 - 400.0,
                (index / columns) as f64 * 8.0 - 400.0,
            );
            if index % 10 == 0 {
                scene.add(
                    Circle::new(progress.map(|p| 3.0 + p * 2.0))
                        .position(position),
                )
            } else {
                scene.add(Circle::new(3.0).position(position))
            }
        })
        .collect();
    Objects {
        scene,
        circles,
        progress,
    }
}

/// The authored timeline: staggered parallel tweens, a signal tween, a
/// parallel colour/opacity block, staggered springs and a hold. This is
/// the only function ported to the `Animation` API.
fn animate(objects: Objects) -> CompiledScene {
    let Objects {
        mut scene,
        circles,
        progress,
    } = objects;
    let spring = Spring::timed(ms(400)).bounce(0.4);
    scene.defaults(ms(300), ease::cubic::ease_in_out);
    scene.compile([
        flow(
            ms(1),
            circles.iter().map(|circle| {
                all([
                    circle.position_x().offset(40.0),
                    circle.scale().to(1.5),
                ])
            }),
        ),
        progress.to(1.0).over(ms(500)),
        all(circles.iter().flat_map(|circle| {
            [circle.fill_to(ORANGE), circle.opacity().to(0.6)]
        })),
        flow(
            ms(1),
            circles.iter().map(|circle| {
                circle.shift(-40.0, 0.0).spring(spring)
            }),
        ),
        wait(ms(200)),
    ])
}

fn frame_times(
    duration: Duration,
) -> impl DoubleEndedIterator<Item = Duration> {
    (0..=FRAMES).map(move |frame| duration * frame / FRAMES)
}

fn bench_setup(c: &mut Criterion) {
    let mut group = c.benchmark_group("setup");
    group.sample_size(10);
    for count in SIZES {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("objects", count),
            &count,
            |b, &count| b.iter(|| black_box(objects(count))),
        );
        group.bench_with_input(
            BenchmarkId::new("animate", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || objects(count),
                    |objects| black_box(animate(objects)),
                    BatchSize::PerIteration,
                )
            },
        );
    }
    group.finish();
}

fn bench_playback(c: &mut Criterion) {
    let mut group = c.benchmark_group("playback");
    group.throughput(Throughput::Elements(u64::from(FRAMES) + 1));
    for count in SIZES {
        let mut animation = animate(objects(count));
        let duration = animation.duration();
        group.bench_function(
            BenchmarkId::new("sample_forward", count),
            |b| {
                b.iter(|| {
                    for time in frame_times(duration) {
                        animation.sample(black_box(time));
                    }
                })
            },
        );
        group.bench_function(
            BenchmarkId::new("seek_backward", count),
            |b| {
                b.iter(|| {
                    for time in frame_times(duration).rev() {
                        animation.sample(black_box(time));
                    }
                })
            },
        );
    }
    group.finish();
}

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("render");
    group.throughput(Throughput::Elements(1));
    let size = Size::new(1280.0, 720.0);
    for count in SIZES {
        let mut animation = animate(objects(count));
        animation.sample(animation.duration() / 2);
        let mut renderer = Renderer::default();
        let mut scene = vello::Scene::new();
        group.bench_function(
            BenchmarkId::new("encode", count),
            |b| {
                b.iter(|| {
                    scene.reset();
                    renderer.render(
                        &animation, &mut scene, size, size, 1.0,
                    );
                    black_box(&scene);
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_setup, bench_playback, bench_encode);
criterion_main!(benches);
