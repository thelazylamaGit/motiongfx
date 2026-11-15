//! Sorting algorithms: `BAR_COUNT` bars, coloured by height, start shuffled
//! and are sorted by each algorithm in turn, reshuffling in between.
//!
//! Each algorithm is plain Rust that sorts a slice and reports every pass.
//! The sorts run once while the scene is built; each reported pass becomes
//! one step that moves only the bars whose slot changed. To add an
//! algorithm, write a function with the same signature as [`bubble_sort`]
//! and list it in [`ALGORITHMS`].

use motiongfx::prelude::{ease, ms};
use motiongfx_2d::{
    Color, Rectangle, RectangleHandle, Scene, Scene2dExt, Text, all,
    colors::WHITE, wait,
};
use std::time::Duration;
use vello::peniko::color::{AlphaColor, Hsl};
use vello_winit_motiongfx::Window;

/// Number of bars; any positive count works.
const BAR_COUNT: usize = 128;
/// Seed for the shuffles, so every run shows the same choreography.
const SEED: u64 = 0x5EED;

/// Width the bars spread across, in canvas units.
const SPREAD: f64 = 1200.0;
/// Height of the tallest bar.
const MAX_HEIGHT: f64 = 540.0;
/// The bars' shared bottom edge.
const BASELINE: f64 = -310.0;

/// How long one pass of a sort takes.
const PASS: Duration = ms(50);
/// How long the bars take to scatter before the next algorithm.
const SHUFFLE: Duration = ms(800);

/// Called by a sort with the whole slice after each pass.
type OnPass<'a, T> = &'a mut dyn FnMut(&[T]);

/// A named sort that reports its passes.
struct Algorithm {
    name: &'static str,
    sort: fn(&mut [usize], OnPass<'_, usize>),
}

/// The algorithms to show, in order.
const ALGORITHMS: &[Algorithm] = &[
    Algorithm {
        name: "Bubble sort",
        sort: bubble_sort,
    },
    Algorithm {
        name: "Selection sort",
        sort: selection_sort,
    },
    Algorithm {
        name: "Insertion sort",
        sort: insertion_sort,
    },
];

/// Sweeps adjacent pairs, swapping those out of order; each pass carries
/// the largest remaining value to the end. Stops after a pass with no swaps.
fn bubble_sort<T: Ord>(values: &mut [T], on_pass: OnPass<'_, T>) {
    for end in (1..values.len()).rev() {
        let mut swapped = false;
        for i in 0..end {
            if values[i] > values[i + 1] {
                values.swap(i, i + 1);
                swapped = true;
            }
        }
        if !swapped {
            break;
        }
        on_pass(values);
    }
}

/// Swaps the smallest remaining value into the next slot.
fn selection_sort<T: Ord>(values: &mut [T], on_pass: OnPass<'_, T>) {
    for i in 0..values.len() {
        let smallest = (i..values.len())
            .min_by(|&a, &b| values[a].cmp(&values[b]))
            .expect("the range starts below its end");
        values.swap(i, smallest);
        on_pass(values);
    }
}

/// Moves each value left past the larger values before it.
fn insertion_sort<T: Ord>(values: &mut [T], on_pass: OnPass<'_, T>) {
    for i in 1..values.len() {
        let mut j = i;
        while j > 0 && values[j - 1] > values[j] {
            values.swap(j - 1, j);
            j -= 1;
        }
        on_pass(values);
    }
}

/// Horizontal centre of the bar in `slot`.
fn slot_x(slot: usize) -> f64 {
    let pitch = SPREAD / BAR_COUNT as f64;
    (slot as f64 - (BAR_COUNT as f64 - 1.0) / 2.0) * pitch
}

/// Height of the bar for `value`, from `0..BAR_COUNT`.
fn bar_height(value: usize) -> f64 {
    (value + 1) as f64 / BAR_COUNT as f64 * MAX_HEIGHT
}

/// Colour for `value`: hue follows height, so sorted bars form a rainbow.
fn bar_color(value: usize) -> Color {
    let hue = 360.0 * value as f32 / BAR_COUNT as f32;
    AlphaColor::<Hsl>::new([hue, 80.0, 62.0, 1.0]).convert()
}

/// Moves every bar whose slot differs between `from` and `to`. Returns
/// `None` when nothing moved, so empty passes add no step.
fn move_bars(
    bars: &[RectangleHandle],
    from: &[usize],
    to: &[usize],
    duration: Duration,
) -> Option<motiongfx_2d::Animation> {
    let moves: Vec<_> = to
        .iter()
        .zip(from)
        .enumerate()
        .filter(|(_, (new, old))| new != old)
        .map(|(slot, (&value, _))| {
            bars[value].position_x().to(slot_x(slot)).over(duration)
        })
        .collect();
    (!moves.is_empty()).then(|| all(moves))
}

fn build_animation() -> motiongfx_2d::CompiledScene {
    let mut scene = Scene::new();
    scene.set_background(Color::from_rgb8(12, 15, 23));
    scene.defaults(PASS, ease::cubic::ease_in_out);

    let title = scene.add(
        Text::new(ALGORITHMS[0].name)
            .position((-SPREAD / 2.0, 320.0))
            .font_size(28.0)
            .fill(WHITE),
    );
    scene.add(
        Text::new(format!("{BAR_COUNT} bars"))
            .position((-SPREAD / 2.0, 288.0))
            .font_size(14.0)
            .fill(WHITE),
    );

    let mut rng = SplitMix64(SEED);
    let mut order: Vec<usize> = (0..BAR_COUNT).collect();
    rng.shuffle(&mut order);

    // `bars[value]` is the bar showing `value`, wherever it currently is.
    let mut start_slot = vec![0; BAR_COUNT];
    for (slot, &value) in order.iter().enumerate() {
        start_slot[value] = slot;
    }
    let bars: Vec<_> = start_slot
        .iter()
        .enumerate()
        .map(|(value, &slot)| {
            let height = bar_height(value);
            let width = SPREAD / BAR_COUNT as f64 * 0.75;
            scene.add(
                Rectangle::new(width, height)
                    .position((slot_x(slot), BASELINE + height / 2.0))
                    .fill(bar_color(value)),
            )
        })
        .collect();

    let mut steps = Vec::new();
    for (index, algorithm) in ALGORITHMS.iter().enumerate() {
        if index > 0 {
            let sorted = order.clone();
            rng.shuffle(&mut order);
            steps.push(title.set_content("Shuffling"));
            steps.extend(move_bars(&bars, &sorted, &order, SHUFFLE));
            steps.push(title.set_content(algorithm.name));
        }
        steps.push(wait(ms(500)));

        let mut shown = order.clone();
        (algorithm.sort)(&mut order, &mut |pass| {
            steps.extend(move_bars(&bars, &shown, pass, PASS));
            shown.copy_from_slice(pass);
        });
        debug_assert!(
            order.is_sorted(),
            "{} left bars unsorted",
            algorithm.name
        );
        steps.push(wait(ms(1200)));
    }
    scene.compile(steps)
}

/// A small, seedable pseudo-random generator, so the example needs no
/// random-number dependency.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Fisher–Yates shuffle.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let animation = build_animation();

    Window::new("MotionGfx | Sorting", (1280.0, 720.0)).run(animation)
}

#[cfg(test)]
#[path = "support/headless.rs"]
mod headless;

#[test]
fn builds_and_samples_headlessly() {
    headless::check(build_animation());
}
