//! Sorting algorithms: `BAR_COUNT` bars, coloured by height, start shuffled
//! and are sorted by each algorithm in turn, reshuffling in between.
//!
//! The file reads top to bottom in three parts:
//!
//! 1. **The algorithms.** Each is ordinary Rust that sorts a slice in place
//!    and calls `on_step` after every comparison or swap, with the slot it
//!    is looking at. They know nothing about animation.
//! 2. **Layout.** Small functions that turn a slot or a value into a
//!    position, size or colour.
//! 3. **The scene.** [`build_animation`] creates the bars, runs every sort
//!    once, and turns each reported step into a short animation: the active
//!    bar turns white, holds, then any bars the step swapped move and the
//!    active bar returns to its colour.
//!
//! The sorts run once, while the scene is built. Playback only samples the
//! finished timeline, which is why scrubbing backwards works.
//!
//! To add an algorithm, write a function with the same signature as
//! [`bubble_sort`] and add it to [`ALGORITHMS`].

use motiongfx::prelude::{ease, ms};
use motiongfx_2d::{
    Animation, Color, CompiledScene, Rectangle, RectangleHandle,
    Scene, Scene2dExt, Text, all,
    colors::{WHITE, hsl},
    wait,
};
use std::time::Duration;
use vello_winit_motiongfx::Window;

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Number of bars; any positive count works.
const BAR_COUNT: usize = 128;
/// Seed for the shuffles, so every run shows the same choreography.
const SEED: u64 = 0x5EED;

/// Width the bars spread across, in canvas units.
const SPREAD: f64 = 1200.0;
/// Distance between the centres of neighbouring bars.
const PITCH: f64 = SPREAD / BAR_COUNT as f64;
/// Width of one bar; the rest of the pitch is the gap between bars.
const BAR_WIDTH: f64 = PITCH * 0.75;
/// Height of the tallest bar.
const MAX_HEIGHT: f64 = 540.0;
/// The bars' shared bottom edge (y points up, 0 is the window's centre).
const BASELINE: f64 = -310.0;

/// How long the bars take to slide to their new slots after each step of a
/// sort. `Duration::ZERO` makes them jump there instantly.
const MOVE_TIME: Duration = Duration::ZERO;
/// How long each step's active bar stays highlighted before the bars move.
/// A sort of 128 bars takes several thousand steps, so keep this short.
const STEP_HOLD: Duration = ms(3);
/// Colour of the bar a step is looking at.
const HIGHLIGHT: Color = WHITE;
/// How long the bars take to scatter before the next algorithm.
const SHUFFLE_TIME: Duration = ms(800);
/// Pause before each sort starts.
const PAUSE_BEFORE: Duration = ms(500);
/// Pause to admire the sorted bars before moving on.
const PAUSE_AFTER: Duration = ms(1200);

// ---------------------------------------------------------------------------
// The algorithms
// ---------------------------------------------------------------------------

/// The callback a sort calls after each comparison or swap, with the whole
/// slice in its current order and the slot it is looking at (its "active"
/// slot, which the animation highlights).
///
/// It is a mutable reference to a closure (`&mut dyn FnMut`) rather than a
/// generic `impl FnMut` so every sort has the same concrete type and can be
/// stored as a plain function pointer in [`Algorithm`].
type OnStep<'a, T> = &'a mut dyn FnMut(&[T], usize);

/// A sorting algorithm and the name shown while it runs.
struct Algorithm {
    name: &'static str,
    sort: fn(&mut [usize], OnStep<'_, usize>),
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

/// Sweeps adjacent pairs, swapping those out of order; each sweep carries
/// the largest remaining value to the end. Stops after a sweep with no
/// swaps. Reports every comparison, with the larger value of the pair
/// active.
fn bubble_sort<T: Ord>(values: &mut [T], on_step: OnStep<'_, T>) {
    for end in (1..values.len()).rev() {
        let mut swapped = false;
        for i in 0..end {
            if values[i] > values[i + 1] {
                values.swap(i, i + 1);
                swapped = true;
            }
            // Swapped or not, the larger of the pair is now at `i + 1`.
            on_step(values, i + 1);
        }
        if !swapped {
            break;
        }
    }
}

/// Scans for the smallest remaining value and swaps it into the next slot.
/// Reports each slot the scan looks at, then the swap.
fn selection_sort<T: Ord>(values: &mut [T], on_step: OnStep<'_, T>) {
    for i in 0..values.len() {
        let mut smallest = i;
        for j in i + 1..values.len() {
            on_step(values, j);
            if values[j] < values[smallest] {
                smallest = j;
            }
        }
        values.swap(i, smallest);
        on_step(values, i);
    }
}

/// Takes each value in turn and moves it left past the larger values before
/// it. Reports picking the value up, then each step it takes.
fn insertion_sort<T: Ord>(values: &mut [T], on_step: OnStep<'_, T>) {
    for i in 1..values.len() {
        on_step(values, i);
        let mut j = i;
        while j > 0 && values[j - 1] > values[j] {
            values.swap(j - 1, j);
            j -= 1;
            on_step(values, j);
        }
    }
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Horizontal centre of the bar in `slot`; the middle slot sits at x = 0.
fn slot_x(slot: usize) -> f64 {
    (slot as f64 - (BAR_COUNT as f64 - 1.0) / 2.0) * PITCH
}

/// Height of the bar for `value` (`0..BAR_COUNT`); the smallest is still
/// visible.
fn bar_height(value: usize) -> f64 {
    (value + 1) as f64 / BAR_COUNT as f64 * MAX_HEIGHT
}

/// Colour for `value`: hue follows height, so sorted bars form a rainbow.
fn bar_color(value: usize) -> Color {
    let hue = 360.0 * value as f32 / BAR_COUNT as f32;
    hsl(hue, 1.0, 0.5)
}

// ---------------------------------------------------------------------------
// The scene
// ---------------------------------------------------------------------------

/// Moves each bar from its slot in `from` to its slot in `to`, all at once,
/// over `duration` (zero jumps instantly).
///
/// Both slices list bar values by slot. Returns `None` when nothing moved,
/// such as after a comparison that didn't swap.
fn move_bars(
    bars: &[RectangleHandle],
    from: &[usize],
    to: &[usize],
    duration: Duration,
) -> Option<Animation> {
    let moves: Vec<Animation> = to
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

fn build_animation() -> CompiledScene {
    let mut scene = Scene::new();
    scene.set_background(Color::from_rgb8(12, 15, 23));
    // Every move uses these unless it says otherwise with `.over(..)`.
    scene.defaults(MOVE_TIME, ease::cubic::ease_in_out);

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

    // A bar's value is its rank: 0 is the shortest, `BAR_COUNT - 1` the
    // tallest. `order[slot]` is the value in that slot, left to right.
    let mut rng = SplitMix64(SEED);
    let mut order: Vec<usize> = (0..BAR_COUNT).collect();
    rng.shuffle(&mut order);

    // `order` maps slot -> value; creating the bars needs value -> slot.
    let mut start_slot = vec![0; BAR_COUNT];
    for (slot, &value) in order.iter().enumerate() {
        start_slot[value] = slot;
    }
    // `bars[value]` is the bar showing `value`, wherever it is on screen.
    let bars: Vec<RectangleHandle> = start_slot
        .iter()
        .enumerate()
        .map(|(value, &slot)| {
            let height = bar_height(value);
            scene.add(
                Rectangle::new(BAR_WIDTH, height)
                    .position((slot_x(slot), BASELINE + height / 2.0))
                    .fill(bar_color(value)),
            )
        })
        .collect();

    // The order as last animated. Each step moves bars from `shown` to the
    // new order, then updates `shown`; one buffer is reused throughout.
    let mut shown = order.clone();
    let mut steps = Vec::new();
    for (index, algorithm) in ALGORITHMS.iter().enumerate() {
        if index > 0 {
            rng.shuffle(&mut order);
            steps.push(title.set_content("Shuffling"));
            steps.extend(move_bars(
                &bars,
                &shown,
                &order,
                SHUFFLE_TIME,
            ));
            shown.copy_from_slice(&order);
        }
        steps.push(title.set_content(algorithm.name));
        steps.push(wait(PAUSE_BEFORE));

        // The sort owns `order` mutably for the call; the closure borrows
        // `bars`, `shown` and `steps`, which are separate variables, so the
        // borrows don't overlap.
        (algorithm.sort)(&mut order, &mut |current, active| {
            let value = current[active];
            let bar = bars[value];
            // Highlight the active bar where it was, hold, move whatever
            // this step swapped, then return the bar to its own colour.
            steps.push(bar.set_fill(HIGHLIGHT));
            steps.push(wait(STEP_HOLD));
            steps
                .extend(move_bars(&bars, &shown, current, MOVE_TIME));
            steps.push(bar.set_fill(bar_color(value)));
            shown.copy_from_slice(current);
        });
        debug_assert!(
            order.is_sorted(),
            "{} left bars unsorted",
            algorithm.name
        );

        steps.push(wait(ms(200)));

        // Sorted now, so value order is left-to-right order: sweep across.
        for (value, bar) in bars.iter().enumerate() {
            steps.extend([
                bar.set_fill(HIGHLIGHT),
                wait(ms(15)),
                bar.set_fill(bar_color(value)),
            ]);
        }
        steps.push(wait(PAUSE_AFTER));
    }
    scene.compile(steps)
}

/// A small, seedable pseudo-random generator, so the example needs no
/// random-number dependency. Not suitable for anything security-related.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Fisher–Yates shuffle: every ordering is (almost exactly) equally
    /// likely.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            // The result is at most `i`, so converting back to `usize`
            // can't truncate.
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
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
