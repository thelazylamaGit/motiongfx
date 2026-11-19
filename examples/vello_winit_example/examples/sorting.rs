//! Sorting algorithms: `BAR_COUNT` bars, coloured by height, start shuffled
//! and are sorted by each algorithm in turn, reshuffling in between.
//!
//! The file reads top to bottom in three parts:
//!
//! 1. **The algorithms.** Each is ordinary Rust that sorts a slice in place
//!    and calls `on_step` after every comparison or swap, with the two
//!    slots involved. They know nothing about animation.
//! 2. **Layout.** Small functions that turn a slot or a value into a
//!    position, size or colour.
//! 3. **The scene.** [`build_animation`] creates the bars, runs every sort
//!    once, and animates each reported step: both bars turn white for a
//!    moment and play a short note whose pitch follows their height, and
//!    any bars the step swapped start sliding to their new slots at the
//!    same time. Slides never hold up the steps; a bar that swaps again
//!    mid-slide turns towards its new slot from wherever it is.
//!
//! The sorts run once, while the scene is built. Playback only samples the
//! finished timeline, which is why scrubbing backwards works. Sound plays
//! only while time moves forwards.
//!
//! To add an algorithm, write a function with the same signature as
//! [`bubble_sort`] and add it to [`ALGORITHMS`].

use motiongfx::prelude::{ease, ms};
use motiongfx_2d::{
    Animation, Color, CompiledScene, Handle, Rectangle, Scene, Text,
    all, chain,
    colors::{WHITE, hsl},
    flow, wait,
};
use motiongfx_audio::Instrument;
use motiongfx_audio::fundsp::prelude32::{adsr_live, sine_hz};
use std::time::Duration;
use vello_winit_example::Window;

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Number of bars; any positive count works.
const BAR_COUNT: usize = 64;
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

/// How long a swapped bar takes to slide to its new slot. Slides run
/// alongside the steps, so this doesn't lengthen the sort.
const MOVE_TIME: Duration = ms(100);
/// Time between steps. A sort takes thousands of steps, so this sets its
/// length.
const STEP_HOLD: Duration = ms(20);
/// How long a bar takes to turn white when the cursor reaches it.
const LIGHT_TIME: Duration = ms(40);
/// How long it then takes to fade back to its own colour, leaving a short
/// trail behind the cursor.
const FADE_TIME: Duration = ms(100);
/// How long the bars take to scatter before the next algorithm.
const SHUFFLE_TIME: Duration = ms(800);
/// Pause before each sort starts.
const PAUSE_BEFORE: Duration = ms(500);
/// Pause to admire the sorted bars before moving on.
const PAUSE_AFTER: Duration = ms(600);

/// How long each step's note is held. Steps are `STEP_HOLD` apart, so
/// several notes overlap; each rings out on its own.
const NOTE_TIME: Duration = ms(80);
/// Notes of the major pentatonic scale, in semitones above the octave's
/// first note. Any two of them sound pleasant together.
const PENTATONIC: [u8; 5] = [0, 2, 4, 7, 9];
/// The shortest bar's note: C3, as a MIDI note number.
const LOWEST_NOTE: u8 = 48;
/// How many scale notes the bars span (three octaves).
const SCALE_NOTES: usize = 15;
/// Each note's peak level. Many notes overlap, so this stays low.
const VOLUME: f32 = 0.1;

// ---------------------------------------------------------------------------
// The algorithms
// ---------------------------------------------------------------------------

/// The two slots one step of a sort looked at, after any swap. Each
/// algorithm decides which is which; the animation shows every step the
/// same way.
#[derive(Clone, Copy)]
struct Step {
    /// The slot holding the value the algorithm is working on: the one
    /// being carried, scanned or inserted.
    active: usize,
    /// The slot it was compared with. Equal to `active` when a value is
    /// swapped with itself.
    compared: usize,
}

/// The callback a sort calls after each comparison (and its swap, if any),
/// with the whole slice in its current order and the [`Step`].
///
/// It is a mutable reference to a closure (`&mut dyn FnMut`) rather than a
/// generic `impl FnMut` so every sort has the same concrete type and can be
/// stored as a plain function pointer in [`Algorithm`].
type OnStep<'a, T> = &'a mut dyn FnMut(&[T], Step);

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
    Algorithm {
        name: "Comb sort",
        sort: comb_sort,
    },
    Algorithm {
        name: "Quick sort",
        sort: quick_sort,
    },
    Algorithm {
        name: "Bitonic sort",
        sort: bitonic_sort,
    },
];

/// Sweeps adjacent pairs, swapping those out of order; each sweep carries
/// the largest remaining value to the end. Stops after a sweep with no
/// swaps. Reports every comparison: the larger of the pair, which the
/// sweep carries on, is active.
fn bubble_sort<T: Ord>(values: &mut [T], on_step: OnStep<'_, T>) {
    for end in (1..values.len()).rev() {
        let mut swapped = false;
        for i in 0..end {
            if values[i] > values[i + 1] {
                values.swap(i, i + 1);
                swapped = true;
            }
            // Swapped or not, the larger of the pair is now at `i + 1`.
            on_step(
                values,
                Step {
                    active: i + 1,
                    compared: i,
                },
            );
        }
        if !swapped {
            break;
        }
    }
}

/// Scans for the smallest remaining value and swaps it into the next slot.
/// Reports each slot the scan looks at (active) against the smallest so far
/// (compared), then the swap: the slot being filled is active.
fn selection_sort<T: Ord>(values: &mut [T], on_step: OnStep<'_, T>) {
    for i in 0..values.len() {
        let mut smallest = i;
        for j in i + 1..values.len() {
            on_step(
                values,
                Step {
                    active: j,
                    compared: smallest,
                },
            );
            if values[j] < values[smallest] {
                smallest = j;
            }
        }
        values.swap(i, smallest);
        // Both slots are `i` when the smallest was already in place.
        on_step(
            values,
            Step {
                active: i,
                compared: smallest,
            },
        );
    }
}

/// Takes each value in turn and moves it left past the larger values before
/// it. Reports every comparison with its left neighbour: the value being
/// inserted is active, compared with each larger value it passes, then
/// with the one that stops it.
fn insertion_sort<T: Ord>(values: &mut [T], on_step: OnStep<'_, T>) {
    for i in 1..values.len() {
        let mut j = i;
        while j > 0 {
            let out_of_order = values[j - 1] > values[j];
            if out_of_order {
                values.swap(j - 1, j);
            }
            // After a swap the inserted value is at `j - 1`, and the
            // larger value it passed at `j`.
            let step = if out_of_order {
                Step {
                    active: j - 1,
                    compared: j,
                }
            } else {
                Step {
                    active: j,
                    compared: j - 1,
                }
            };
            on_step(values, step);
            if !out_of_order {
                break;
            }
            j -= 1;
        }
    }
}

/// Bubble sort with a shrinking gap: compares values `gap` slots apart and
/// swaps those out of order, so small values near the end jump forward
/// instead of creeping one slot per sweep. The gap starts at the slice's
/// length and shrinks by about 1.3 each sweep; at 1 it is a bubble sort,
/// and it stops after a sweep with no swaps. Reports every comparison: the
/// larger of the pair, which moves on, is active.
fn comb_sort<T: Ord>(values: &mut [T], on_step: OnStep<'_, T>) {
    let mut gap = values.len();
    let mut swapped = true;
    while gap > 1 || swapped {
        // Dividing by 1.3 works well in practice; integer maths avoids
        // rounding a float back to a slot count.
        gap = (gap * 10 / 13).max(1);
        swapped = false;
        // Saturating, so an empty slice has no pairs instead of
        // underflowing.
        for i in 0..values.len().saturating_sub(gap) {
            if values[i] > values[i + gap] {
                values.swap(i, i + gap);
                swapped = true;
            }
            // Swapped or not, the larger of the pair is now at `i + gap`.
            on_step(
                values,
                Step {
                    active: i + gap,
                    compared: i,
                },
            );
        }
    }
}

/// Picks the last value of a range as the pivot, moves everything no larger
/// than it to its left, puts it between the two groups, and sorts each group
/// the same way. Reports every comparison with the pivot: the scanned value
/// is active. Then it reports the pivot moving into place, as active.
fn quick_sort<T: Ord>(values: &mut [T], on_step: OnStep<'_, T>) {
    let len = values.len();
    quick_sort_range(values, 0, len, on_step);
}

/// Sorts `values[lo..hi]` and leaves the rest alone. It still gets the whole
/// slice so the slots it reports are slots of the whole slice.
fn quick_sort_range<T: Ord>(
    values: &mut [T],
    lo: usize,
    hi: usize,
    on_step: OnStep<'_, T>,
) {
    if hi - lo <= 1 {
        return;
    }
    let pivot = partition(values, lo, hi, on_step);
    quick_sort_range(values, lo, pivot, on_step);
    quick_sort_range(values, pivot + 1, hi, on_step);
}

/// Splits `values[lo..hi]` around its last value and returns the pivot's
/// final slot.
fn partition<T: Ord>(
    values: &mut [T],
    lo: usize,
    hi: usize,
    on_step: OnStep<'_, T>,
) -> usize {
    let pivot = hi - 1;
    // The next slot for a value no larger than the pivot.
    let mut store = lo;
    for j in lo..pivot {
        let smaller = values[j] <= values[pivot];
        if smaller {
            values.swap(store, j);
        }
        // After a swap the scanned value is at `store`.
        on_step(
            values,
            Step {
                active: if smaller { store } else { j },
                compared: pivot,
            },
        );
        if smaller {
            store += 1;
        }
    }
    values.swap(store, pivot);
    // Both slots are `store` when the pivot was already in place.
    on_step(
        values,
        Step {
            active: store,
            compared: pivot,
        },
    );
    store
}

/// Sorts the two halves in opposite directions, then merges them. A merge
/// compares pairs a fixed distance apart, so every comparison is chosen in
/// advance and never depends on the values. That is why it runs well on
/// parallel hardware. Splitting at the largest power of two below the length
/// lets it sort any length, not just powers of two. Reports every
/// comparison: the larger of the pair is active.
fn bitonic_sort<T: Ord>(values: &mut [T], on_step: OnStep<'_, T>) {
    let len = values.len();
    bitonic_sort_range(values, 0, len, true, on_step);
}

/// Sorts `values[lo..hi]` ascending or descending and leaves the rest alone.
fn bitonic_sort_range<T: Ord>(
    values: &mut [T],
    lo: usize,
    hi: usize,
    ascending: bool,
    on_step: OnStep<'_, T>,
) {
    if hi - lo <= 1 {
        return;
    }
    let mid = lo + (hi - lo) / 2;
    // Opposite directions make the whole range rise then fall (or fall
    // then rise), which is the shape a bitonic merge sorts.
    bitonic_sort_range(values, lo, mid, !ascending, on_step);
    bitonic_sort_range(values, mid, hi, ascending, on_step);
    bitonic_merge(values, lo, hi, ascending, on_step);
}

/// Sorts `values[lo..hi]`, which rises then falls (or the reverse), in the
/// given direction.
fn bitonic_merge<T: Ord>(
    values: &mut [T],
    lo: usize,
    hi: usize,
    ascending: bool,
    on_step: OnStep<'_, T>,
) {
    let len = hi - lo;
    if len <= 1 {
        return;
    }
    // The largest power of two below `len`: half of it when `len` is itself
    // a power of two.
    let gap = 1 << (len - 1).ilog2();
    for i in lo..hi - gap {
        let j = i + gap;
        let out_of_order = if ascending {
            values[i] > values[j]
        } else {
            values[i] < values[j]
        };
        if out_of_order {
            values.swap(i, j);
        }
        // An ascending merge leaves the larger value at `j`, a descending
        // one at `i`.
        let (active, compared) =
            if ascending { (j, i) } else { (i, j) };
        on_step(values, Step { active, compared });
    }
    bitonic_merge(values, lo, lo + gap, ascending, on_step);
    bitonic_merge(values, lo + gap, hi, ascending, on_step);
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
    // oklch(0.7, 0.3, hue)
}

/// Note for `value`: taller bars play higher notes of the pentatonic
/// scale. With more bars than scale notes, neighbouring values share one.
fn bar_note(value: usize) -> u8 {
    let degree = value * SCALE_NOTES / BAR_COUNT;
    // `degree` is below `SCALE_NOTES`, so the octave fits in a `u8`.
    let octave = (degree / PENTATONIC.len()) as u8;
    LOWEST_NOTE + 12 * octave + PENTATONIC[degree % PENTATONIC.len()]
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
    bars: &[Handle<Rectangle>],
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

/// Builds the scene, playing each step's note on `instrument`.
fn build_animation(instrument: &Instrument) -> CompiledScene {
    let mut scene = Scene::new();
    scene.set_background(Color::from_rgb8(12, 15, 23));
    // Every move uses these unless it says otherwise. Ease-out starts at
    // full speed: an interrupted slide restarts from rest, so with an
    // ease-in curve a bar that keeps swapping would barely move until
    // its last swap, then rush the whole way.
    scene.defaults(MOVE_TIME, ease::cubic::ease_out);

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
    let mut start_slot = [0; BAR_COUNT];
    for (slot, &value) in order.iter().enumerate() {
        start_slot[value] = slot;
    }
    // `bars[value]` is the bar showing `value`, wherever it is on screen.
    let bars: Vec<Handle<Rectangle>> = start_slot
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

        // One animation per step of the sort, started `STEP_HOLD` apart.
        let mut sort = Vec::new();
        // The sort owns `order` mutably for the call; the closure borrows
        // `bars`, `shown` and `sort`, which are separate variables, so the
        // borrows don't overlap.
        (algorithm.sort)(&mut order, &mut |current, step| {
            // A slot swapped with itself lights up once.
            let lit = [step.active, step.compared];
            let lit = if step.active == step.compared {
                &lit[..1]
            } else {
                &lit[..]
            };
            // Each lit bar turns white, fades back and plays its note,
            // while any swapped bars slide to their new slots.
            let visits = lit.iter().map(|&slot| {
                let value = current[slot];
                let bar = bars[value];
                all([
                    chain([
                        bar.fill_to(WHITE).over(LIGHT_TIME),
                        bar.fill_to(bar_color(value)).over(FADE_TIME),
                    ]),
                    instrument.note(bar_note(value)).over(NOTE_TIME),
                ])
            });
            let slides = move_bars(&bars, &shown, current, MOVE_TIME);
            // Steps come faster than fades and slides finish, so a step
            // takes over a bar from where the last one left it: a bar
            // lit again mid-fade lights up from there, and a bar swapped
            // again mid-slide turns towards its new slot. Notes always
            // ring out.
            sort.push(all(visits.chain(slides)).interrupt());
            shown.copy_from_slice(current);
        });
        steps.push(flow(STEP_HOLD, sort));
        debug_assert!(
            order.is_sorted(),
            "{} left bars unsorted",
            algorithm.name
        );

        steps.push(wait(ms(200)));

        // Sorted now, so value order is left-to-right order: sweep across,
        // climbing the scale. The `any` moves on after 15 ms while the
        // fade and the note carry on.
        steps.push(flow(
            ms(15),
            bars.iter().enumerate().map(|(value, bar)| {
                all([
                    chain([
                        bar.set_fill(WHITE),
                        bar.fill_to(bar_color(value)).over(FADE_TIME),
                    ]),
                    instrument.note(bar_note(value)).over(NOTE_TIME),
                ])
            }),
        ));
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

/// A soft sine: a quick attack, then it settles to 40% while held and
/// fades over 100 ms once released. Scaled down because many notes
/// overlap.
fn sine_instrument() -> Instrument {
    Instrument::synth(|hz| {
        VOLUME * (sine_hz(hz) * adsr_live(0.005, 0.05, 0.4, 0.1))
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Window::new("MotionGfx | Sorting", (1280.0, 720.0))
        .audio()
        .run(build_animation(&sine_instrument()))?;
    Ok(())
}

#[cfg(test)]
#[path = "support/headless.rs"]
mod headless;

#[test]
fn builds_and_samples_headlessly() {
    headless::check(build_animation(&sine_instrument()));
}
