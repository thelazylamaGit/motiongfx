//! Sorting algorithms: `BAR_COUNT` bars, coloured by height, start shuffled
//! and are sorted by each algorithm in turn, reshuffling in between.
//!
//! The file reads top to bottom in three parts:
//!
//! 1. **The algorithms.** Each is ordinary Rust that sorts a [`Tracked`]
//!    array through its methods: comparing, swapping and reading values,
//!    and moving them to and from a scratch buffer. Each call also records
//!    an [`Op`]. The algorithms know nothing about animation.
//! 2. **Layout.** Small functions that turn a slot or a value into a
//!    position, size or colour.
//! 3. **The scene.** [`build_animation`] creates the bars, runs every sort
//!    once, and plays back its ops in beats. A swap or move shares a beat
//!    with the comparison or read that decided it. Compared and read bars
//!    turn white for a moment and play a note whose pitch follows their
//!    height. Swapped bars slide or exchange heights ([`SWAP_STYLE`]), and
//!    moved bars drop into a buffer row below the main one or, with the
//!    buffer hidden, overwrite the bar in their slot ([`BUFFER_STYLE`]).
//!    Moves never hold up the beats; a bar that moves again mid-slide
//!    turns towards its new place from wherever it is.
//!
//! The sorts run once, while the scene is built. Playback only samples the
//! finished timeline, which is why scrubbing backwards works. Sound plays
//! only while time moves forwards.
//!
//! To add an algorithm, write a function with the same signature as
//! [`bubble_sort`] and add it to [`ALGORITHMS`].

use crate::Loc::{Buffer, Main};
use motiongfx::prelude::{ease, ms, s};
use motiongfx_2d::{
    Animation, Color, CompiledScene, Handle, Point, Rectangle, Scene,
    Size, Text, all, chain,
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

/// How swapped bars change places.
const SWAP_STYLE: SwapStyle = SwapStyle::Slide;
/// Whether sorts that use a scratch buffer show it as a second row.
const BUFFER_STYLE: BufferStyle = BufferStyle::Row;

/// Width the bars spread across, in canvas units.
const SPREAD: f64 = 1200.0;
/// Distance between the centres of neighbouring bars.
const PITCH: f64 = SPREAD / BAR_COUNT as f64;
/// Width of one bar; the rest of the pitch is the gap between bars.
const BAR_WIDTH: f64 = PITCH * 0.75;
/// The bars' row while no buffer row is shown (y points up, 0 is the
/// window's centre).
const FULL_ROW: RowLayout = RowLayout {
    baseline: -310.0,
    max_height: 540.0,
};
/// The main row while a buffer row is shown below it.
const MAIN_ROW: RowLayout = RowLayout {
    baseline: -10.0,
    max_height: 260.0,
};
/// The buffer row, below the main one.
const BUFFER_ROW: RowLayout = RowLayout {
    baseline: -310.0,
    max_height: 260.0,
};

/// About how long each sort takes. A beat starts every `SORT_TIME` divided
/// by the sort's number of beats, so sorts that do more work go faster,
/// within `MIN_BEAT..=MAX_BEAT`.
const SORT_TIME: Duration = s(15);
/// The shortest time between beats.
const MIN_BEAT: Duration = ms(5);
/// The longest time between beats.
const MAX_BEAT: Duration = ms(50);
/// How long a swapped or moved bar takes to reach its new place. Moves run
/// alongside the beats, so this doesn't lengthen the sort.
const MOVE_TIME: Duration = ms(100);
/// How long a bar takes to turn white when the cursor reaches it.
const LIGHT_TIME: Duration = ms(40);
/// How long it then takes to fade back to its own colour, leaving a short
/// trail behind the cursor.
const FADE_TIME: Duration = ms(100);
/// How long the rows take to make room for the buffer row, and to close
/// it again.
const SPLIT_TIME: Duration = ms(500);
/// How long the bars take to scatter before the next algorithm.
const SHUFFLE_TIME: Duration = ms(800);
/// Pause before each sort starts.
const PAUSE_BEFORE: Duration = ms(500);
/// Pause to admire the sorted bars before moving on.
const PAUSE_AFTER: Duration = ms(600);

/// How long each lit bar's note is held. Several notes overlap; each rings
/// out on its own.
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

/// A sorting algorithm and the name shown while it runs.
struct Algorithm {
    name: &'static str,
    sort: fn(&mut Tracked),
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
    Algorithm {
        name: "Radix sort (MSD)",
        sort: radix_msd_sort,
    },
    Algorithm {
        name: "Merge sort",
        sort: merge_sort,
    },
    Algorithm {
        name: "Counting sort",
        sort: counting_sort,
    },
    Algorithm {
        name: "Radix sort (LSD, base 4)",
        sort: radix_lsd_sort,
    },
];

/// A place that holds one value: a slot of the main row, or of the scratch
/// buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Loc {
    Main(usize),
    Buffer(usize),
}

/// One thing a sort did, recorded by [`Tracked`] for the scene to animate.
#[derive(Clone, Copy, Debug)]
enum Op {
    /// Compared the values at two places.
    Compare(Loc, Loc),
    /// Read one value as a key, for sorts that use its digits or count it
    /// instead of comparing it.
    Read(Loc),
    /// Swapped two slots of the main row.
    Swap(usize, usize),
    /// Moved `value` from one place to an empty one, leaving `from` empty.
    Move { from: Loc, to: Loc, value: usize },
}

/// The values a sort works on: the main row, and a scratch buffer of the
/// same length that starts empty. Every comparison, read, swap and move
/// through its methods is recorded as an [`Op`].
struct Tracked {
    main: Vec<Option<usize>>,
    buffer: Vec<Option<usize>>,
    ops: Vec<Op>,
}

impl Tracked {
    fn new(values: &[usize]) -> Self {
        Self {
            main: values.iter().copied().map(Some).collect(),
            buffer: vec![None; values.len()],
            ops: Vec::new(),
        }
    }

    /// Number of slots in the main row, and in the buffer.
    fn len(&self) -> usize {
        self.main.len()
    }

    fn slot(&mut self, at: Loc) -> &mut Option<usize> {
        match at {
            Main(slot) => &mut self.main[slot],
            Buffer(slot) => &mut self.buffer[slot],
        }
    }

    /// The value at `at`, without recording anything.
    ///
    /// # Panics
    ///
    /// If `at` is empty because its value was moved away.
    #[track_caller]
    fn value(&mut self, at: Loc) -> usize {
        match *self.slot(at) {
            Some(value) => value,
            None => panic!("{at:?} is empty"),
        }
    }

    /// Whether the value in main slot `i` is smaller than the one in `j`.
    #[track_caller]
    fn less(&mut self, i: usize, j: usize) -> bool {
        self.less_at(Main(i), Main(j))
    }

    /// Whether the value at `a` is smaller than the one at `b`.
    #[track_caller]
    fn less_at(&mut self, a: Loc, b: Loc) -> bool {
        self.ops.push(Op::Compare(a, b));
        self.value(a) < self.value(b)
    }

    /// The value at `at`, to use as a key: its digits, or which counter it
    /// adds to.
    #[track_caller]
    fn read(&mut self, at: Loc) -> usize {
        self.ops.push(Op::Read(at));
        self.value(at)
    }

    /// Swaps main slots `i` and `j`. Swapping a slot with itself changes
    /// nothing, so it isn't recorded.
    fn swap(&mut self, i: usize, j: usize) {
        if i != j {
            self.main.swap(i, j);
            self.ops.push(Op::Swap(i, j));
        }
    }

    /// Moves the value at `from` to the empty place `to`.
    ///
    /// # Panics
    ///
    /// If `from` is empty or `to` isn't.
    #[track_caller]
    fn move_value(&mut self, from: Loc, to: Loc) {
        let value = self.value(from);
        if self.slot(to).is_some() {
            panic!("{to:?} is not empty");
        }
        *self.slot(from) = None;
        *self.slot(to) = Some(value);
        self.ops.push(Op::Move { from, to, value });
    }

    /// The sorted values and the ops that sorted them.
    ///
    /// # Panics
    ///
    /// If the sort left a value in the buffer.
    fn finish(self) -> (Vec<usize>, Vec<Op>) {
        let values = self
            .main
            .into_iter()
            .map(|value| {
                value.expect("the sort left a value in the buffer")
            })
            .collect();
        (values, self.ops)
    }
}

/// Sweeps adjacent pairs, swapping those out of order; each sweep carries
/// the largest remaining value to the end. Stops after a sweep with no
/// swaps.
fn bubble_sort(a: &mut Tracked) {
    for end in (1..a.len()).rev() {
        let mut swapped = false;
        for i in 0..end {
            if a.less(i + 1, i) {
                a.swap(i, i + 1);
                swapped = true;
            }
        }
        if !swapped {
            break;
        }
    }
}

/// Scans for the smallest remaining value and swaps it into the next slot.
fn selection_sort(a: &mut Tracked) {
    for i in 0..a.len() {
        let mut smallest = i;
        for j in i + 1..a.len() {
            if a.less(j, smallest) {
                smallest = j;
            }
        }
        a.swap(i, smallest);
    }
}

/// Takes each value in turn and swaps it left past the larger values
/// before it.
fn insertion_sort(a: &mut Tracked) {
    for i in 1..a.len() {
        let mut j = i;
        while j > 0 && a.less(j, j - 1) {
            a.swap(j, j - 1);
            j -= 1;
        }
    }
}

/// Bubble sort with a shrinking gap: compares values `gap` slots apart and
/// swaps those out of order, so small values near the end jump forward
/// instead of creeping one slot per sweep. The gap starts at the length and
/// shrinks by about 1.3 each sweep; at 1 it is a bubble sort, and it stops
/// after a sweep with no swaps.
fn comb_sort(a: &mut Tracked) {
    let mut gap = a.len();
    let mut swapped = true;
    while gap > 1 || swapped {
        // Dividing by 1.3 works well in practice; integer maths avoids
        // rounding a float back to a slot count.
        gap = (gap * 10 / 13).max(1);
        swapped = false;
        // Saturating, so an empty array has no pairs instead of
        // underflowing.
        for i in 0..a.len().saturating_sub(gap) {
            if a.less(i + gap, i) {
                a.swap(i, i + gap);
                swapped = true;
            }
        }
    }
}

/// Picks the last value of a range as the pivot, moves everything no
/// larger than it to its left, puts it between the two groups, and sorts
/// each group the same way.
fn quick_sort(a: &mut Tracked) {
    let len = a.len();
    quick_sort_range(a, 0, len);
}

/// Sorts main slots `lo..hi` and leaves the rest alone.
fn quick_sort_range(a: &mut Tracked, lo: usize, hi: usize) {
    if hi - lo <= 1 {
        return;
    }
    let pivot = partition(a, lo, hi);
    quick_sort_range(a, lo, pivot);
    quick_sort_range(a, pivot + 1, hi);
}

/// Splits main slots `lo..hi` around the last one's value and returns the
/// pivot's final slot.
fn partition(a: &mut Tracked, lo: usize, hi: usize) -> usize {
    let pivot = hi - 1;
    // The next slot for a value no larger than the pivot.
    let mut store = lo;
    for j in lo..pivot {
        if !a.less(pivot, j) {
            a.swap(store, j);
            store += 1;
        }
    }
    a.swap(store, pivot);
    store
}

/// Sorts the two halves in opposite directions, then merges them. A merge
/// compares pairs a fixed distance apart, so every comparison is chosen in
/// advance and never depends on the values. That is why it runs well on
/// parallel hardware. Splitting at the largest power of two below the
/// length lets it sort any length, not just powers of two.
fn bitonic_sort(a: &mut Tracked) {
    let len = a.len();
    bitonic_sort_range(a, 0, len, true);
}

/// Sorts main slots `lo..hi` ascending or descending and leaves the rest
/// alone.
fn bitonic_sort_range(
    a: &mut Tracked,
    lo: usize,
    hi: usize,
    ascending: bool,
) {
    if hi - lo <= 1 {
        return;
    }
    let mid = lo + (hi - lo) / 2;
    // Opposite directions make the whole range rise then fall (or fall
    // then rise), which is the shape a bitonic merge sorts.
    bitonic_sort_range(a, lo, mid, !ascending);
    bitonic_sort_range(a, mid, hi, ascending);
    bitonic_merge(a, lo, hi, ascending);
}

/// Sorts main slots `lo..hi`, which rise then fall (or the reverse), in
/// the given direction.
fn bitonic_merge(
    a: &mut Tracked,
    lo: usize,
    hi: usize,
    ascending: bool,
) {
    let len = hi - lo;
    if len <= 1 {
        return;
    }
    // The largest power of two below `len`: half of it when `len` is
    // itself a power of two.
    let gap = 1 << (len - 1).ilog2();
    for i in lo..hi - gap {
        let j = i + gap;
        let out_of_order = if ascending {
            a.less(j, i)
        } else {
            a.less(i, j)
        };
        if out_of_order {
            a.swap(i, j);
        }
    }
    bitonic_merge(a, lo, lo + gap, ascending);
    bitonic_merge(a, lo + gap, hi, ascending);
}

/// Reads every value to find the largest. `None` if there are none.
fn max_value(a: &mut Tracked) -> Option<usize> {
    let len = a.len();
    (0..len).map(|slot| a.read(Main(slot))).max()
}

/// Sorts by binary digit, starting from the highest bit any value uses:
/// values with that bit clear go left, values with it set go right, and
/// each side is then sorted by the next bit down. It never compares two
/// values, only reads their bits.
fn radix_msd_sort(a: &mut Tracked) {
    // Bits above the largest value's highest set bit are clear everywhere.
    let Some(top_bit) = max_value(a).and_then(usize::checked_ilog2)
    else {
        return; // Empty, or all zeros: already sorted.
    };
    let len = a.len();
    radix_msd_range(a, 0, len, top_bit);
}

/// Sorts main slots `lo..hi` by `bit` and the bits below it, and leaves the
/// rest alone.
fn radix_msd_range(a: &mut Tracked, lo: usize, hi: usize, bit: u32) {
    if hi - lo <= 1 {
        return;
    }
    let mask = 1 << bit;
    // `lo..zeros` holds values with the bit clear and `ones..hi` values
    // with it set. Each read shrinks the unread middle by one slot.
    let mut zeros = lo;
    let mut ones = hi;
    while zeros < ones {
        if a.read(Main(zeros)) & mask == 0 {
            zeros += 1;
        } else {
            ones -= 1;
            // The unread value from `ones` now waits at `zeros`, to be
            // read next.
            a.swap(zeros, ones);
        }
    }
    if bit > 0 {
        radix_msd_range(a, lo, zeros, bit - 1);
        radix_msd_range(a, zeros, hi, bit - 1);
    }
}

/// Sorts each half, then merges the two sorted halves: it moves both into
/// the buffer, then repeatedly moves the smaller of their first values
/// back. Equal values keep their order, so the sort is stable.
fn merge_sort(a: &mut Tracked) {
    let len = a.len();
    merge_sort_range(a, 0, len);
}

/// Sorts main slots `lo..hi` and leaves the rest alone.
fn merge_sort_range(a: &mut Tracked, lo: usize, hi: usize) {
    if hi - lo <= 1 {
        return;
    }
    let mid = lo + (hi - lo) / 2;
    merge_sort_range(a, lo, mid);
    merge_sort_range(a, mid, hi);
    for slot in lo..hi {
        a.move_value(Main(slot), Buffer(slot));
    }
    // The first value not yet moved back from each half.
    let mut left = lo;
    let mut right = mid;
    for slot in lo..hi {
        // Taking the left value on a tie keeps the sort stable.
        let take_left = right == hi
            || (left < mid
                && !a.less_at(Buffer(right), Buffer(left)));
        let from = if take_left { &mut left } else { &mut right };
        a.move_value(Buffer(*from), Main(slot));
        *from += 1;
    }
}

/// Counts how many times each value appears, which gives each value's
/// first slot in the sorted order, then moves every value, in order, into
/// the next free slot for it. It never compares two values; it needs one
/// counter per value up to the largest.
fn counting_sort(a: &mut Tracked) {
    let Some(max) = max_value(a) else {
        return;
    };
    sort_by_key_counts(a, max + 1, |value| value);
}

/// Counting sort on one base-4 digit at a time, from the lowest. Each pass
/// splits the buffer into four buckets, one per digit, side by side.
/// Because each pass is stable, values with the same digit stay in the
/// order the lower digits set, so after the highest digit's pass the row
/// is sorted.
fn radix_lsd_sort(a: &mut Tracked) {
    const RADIX: usize = 4;
    let Some(max) = max_value(a) else {
        return;
    };
    // The place value of the digit this pass sorts by: 1, 4, 16, ...
    let mut place = 1;
    while place <= max {
        sort_by_key_counts(a, RADIX, |value| value / place % RADIX);
        let Some(next) = place.checked_mul(RADIX) else {
            break; // `max` has no higher digit than this one.
        };
        place = next;
    }
}

/// Rearranges the main row so keys ascend, keeping values with equal keys
/// in their current order. `key` maps a value to `0..key_count`.
///
/// Counts each key, which gives each key's first slot. Then it moves every
/// value, in order, into its key's next free slot in the buffer, and moves
/// the buffer back.
fn sort_by_key_counts(
    a: &mut Tracked,
    key_count: usize,
    key: impl Fn(usize) -> usize,
) {
    let mut next_slot = vec![0; key_count];
    for slot in 0..a.len() {
        next_slot[key(a.read(Main(slot)))] += 1;
    }
    // Each key's first slot comes after every smaller key's values.
    let mut start = 0;
    for count in &mut next_slot {
        let key_total = *count;
        *count = start;
        start += key_total;
    }
    for slot in 0..a.len() {
        let bucket = &mut next_slot[key(a.read(Main(slot)))];
        a.move_value(Main(slot), Buffer(*bucket));
        *bucket += 1;
    }
    for slot in 0..a.len() {
        a.move_value(Buffer(slot), Main(slot));
    }
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// A row of bars: where their bottom edge sits and how tall the tallest is.
#[derive(Clone, Copy)]
struct RowLayout {
    baseline: f64,
    max_height: f64,
}

impl RowLayout {
    /// Centre and size of the bar showing `value` (`0..BAR_COUNT`) in
    /// `slot`. The smallest value is still visible.
    fn place(self, slot: usize, value: usize) -> (Point, Size) {
        let height =
            (value + 1) as f64 / BAR_COUNT as f64 * self.max_height;
        (
            Point::new(slot_x(slot), self.baseline + height / 2.0),
            Size::new(BAR_WIDTH, height),
        )
    }
}

/// Horizontal centre of `slot`; the middle slot sits at x = 0.
fn slot_x(slot: usize) -> f64 {
    (slot as f64 - (BAR_COUNT as f64 - 1.0) / 2.0) * PITCH
}

/// Colour for `value`: hue follows height, so sorted bars form a rainbow.
fn bar_color(value: usize) -> Color {
    let hue = 360.0 * value as f32 / BAR_COUNT as f32;
    hsl(hue, 1.0, 0.5)
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

/// How swapped bars change places.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "the unused style is one edit away")]
enum SwapStyle {
    /// The two bars slide to each other's slots.
    Slide,
    /// The bars stay in their slots and exchange heights and colours.
    Heights,
}

/// How sorts that move values to the scratch buffer show it.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "the unused style is one edit away")]
enum BufferStyle {
    /// A second row below the main one. The main row shrinks to make room
    /// while such a sort runs, and bars moved to the buffer drop into it.
    Row,
    /// Not drawn. A bar moved to the buffer stays where it was, and a value
    /// moved back overwrites the bar in its slot, as in most one-row
    /// visualisers.
    Hidden,
}

/// Whether `op` acts on what `prev` just looked at, such as a swap right
/// after comparing one of the same slots, so the two play as one beat.
fn acts_on(prev: &Op, op: &Op) -> bool {
    let looked = match *prev {
        Op::Compare(a, b) => [a, b],
        Op::Read(at) => [at, at],
        _ => return false,
    };
    let acted = match *op {
        Op::Swap(i, j) => [Main(i), Main(j)],
        Op::Move { from, .. } => [from, from],
        _ => return false,
    };
    acted.iter().any(|at| looked.contains(at))
}

/// What a bar was last animated to, so each beat only animates what
/// changed.
#[derive(Clone, Copy, PartialEq)]
struct Drawn {
    position: Point,
    size: Size,
    value: usize,
}

/// The bars on screen: which value each shows and where it is, kept in
/// step with a sort as its ops are applied.
struct Stage<'a> {
    instrument: &'a Instrument,
    bars: Vec<Handle<Rectangle>>,
    /// The value each bar shows. Bars exchange values rather than places
    /// with `SwapStyle::Heights` or a hidden buffer, so after such a sort
    /// this is no longer each bar's starting value.
    shows: Vec<usize>,
    /// The bar in each main-row slot; `None` while its value is in the
    /// buffer row.
    main: Vec<Option<usize>>,
    /// The bar in each buffer-row slot; always empty with a hidden buffer.
    buffer: Vec<Option<usize>>,
    /// Whether the buffer row is shown, shrinking the main row.
    split: bool,
    /// What each bar was last animated to.
    drawn: Vec<Drawn>,
}

impl<'a> Stage<'a> {
    /// Adds a bar to `scene` for each value in `order`, left to right.
    fn new(
        scene: &mut Scene,
        instrument: &'a Instrument,
        order: &[usize],
    ) -> Self {
        let drawn: Vec<Drawn> = order
            .iter()
            .enumerate()
            .map(|(slot, &value)| {
                let (position, size) = FULL_ROW.place(slot, value);
                Drawn {
                    position,
                    size,
                    value,
                }
            })
            .collect();
        let bars = drawn
            .iter()
            .map(|bar| {
                scene.add(
                    Rectangle::from_size(bar.size)
                        .position(bar.position)
                        .fill(bar_color(bar.value)),
                )
            })
            .collect();
        Self {
            instrument,
            bars,
            shows: order.to_vec(),
            main: (0..order.len()).map(Some).collect(),
            buffer: vec![None; order.len()],
            split: false,
            drawn,
        }
    }

    /// The bar at `at`, if one is drawn there.
    fn bar_at(&mut self, at: Loc) -> &mut Option<usize> {
        match at {
            Main(slot) => &mut self.main[slot],
            Buffer(slot) => &mut self.buffer[slot],
        }
    }

    /// Updates the bars for `op`, adding the bars it lights to `lit`.
    fn apply(&mut self, op: Op, lit: &mut Vec<usize>) {
        match op {
            Op::Compare(a, b) => {
                lit.extend(*self.bar_at(a));
                lit.extend(*self.bar_at(b));
            }
            Op::Read(at) => lit.extend(*self.bar_at(at)),
            Op::Swap(i, j) => match SWAP_STYLE {
                SwapStyle::Slide => self.main.swap(i, j),
                SwapStyle::Heights => {
                    let (a, b) = (self.main[i], self.main[j]);
                    self.shows.swap(
                        a.expect("swapped an empty slot"),
                        b.expect("swapped an empty slot"),
                    );
                }
            },
            Op::Move { from, to, value } => match BUFFER_STYLE {
                BufferStyle::Row => {
                    let bar = self
                        .bar_at(from)
                        .take()
                        .expect("moved from an empty slot");
                    *self.bar_at(to) = Some(bar);
                    lit.push(bar);
                }
                // Bars never leave the main row: one whose value moved to
                // the buffer keeps showing it until another is moved in.
                BufferStyle::Hidden => match to {
                    Main(slot) => {
                        let bar = self.main[slot]
                            .expect("hidden buffer keeps every slot");
                        self.shows[bar] = value;
                        lit.push(bar);
                    }
                    Buffer(_) => lit.extend(*self.bar_at(from)),
                },
            },
        }
    }

    /// Animates every bar to where it now is and what it now shows, taking
    /// `duration`, and lights the bars in `lit`: each turns white, fades to
    /// its colour and plays its note.
    fn sync(
        &mut self,
        duration: Duration,
        lit: &[usize],
    ) -> Vec<Animation> {
        let main_row = if self.split { MAIN_ROW } else { FULL_ROW };
        // Destructured so the closure can update `drawn` while the loops
        // below read `main` and `buffer`.
        let Self {
            instrument,
            bars,
            shows,
            main,
            buffer,
            drawn,
            ..
        } = self;
        let mut animations = Vec::new();
        let mut animate =
            |bar: usize, (position, size): (Point, Size)| {
                let target = Drawn {
                    position,
                    size,
                    value: shows[bar],
                };
                let last = std::mem::replace(&mut drawn[bar], target);
                let handle = bars[bar];
                if target.position != last.position {
                    animations.push(
                        handle
                            .position()
                            .to(target.position)
                            .over(duration),
                    );
                }
                if target.size != last.size {
                    animations.push(
                        handle.size().to(target.size).over(duration),
                    );
                }
                let color = bar_color(target.value);
                if lit.contains(&bar) {
                    animations.push(all([
                        chain([
                            handle.fill_to(WHITE).over(LIGHT_TIME),
                            handle.fill_to(color).over(FADE_TIME),
                        ]),
                        instrument
                            .note(bar_note(target.value))
                            .over(NOTE_TIME),
                    ]));
                } else if target.value != last.value {
                    animations
                        .push(handle.fill_to(color).over(duration));
                }
            };
        for (slot, bar) in main.iter().enumerate() {
            if let Some(bar) = *bar {
                animate(bar, main_row.place(slot, shows[bar]));
            }
        }
        for (slot, bar) in buffer.iter().enumerate() {
            if let Some(bar) = *bar {
                animate(bar, BUFFER_ROW.place(slot, shows[bar]));
            }
        }
        animations
    }

    /// Puts the bars in `order`, left to right, to animate with `sync`.
    /// Every bar must be in the main row.
    fn arrange(&mut self, order: &[usize]) {
        let mut bar_of = vec![0; order.len()];
        for (bar, &value) in self.shows.iter().enumerate() {
            bar_of[value] = bar;
        }
        for (slot, &value) in order.iter().enumerate() {
            self.main[slot] = Some(bar_of[value]);
        }
    }

    /// Whether the main row shows `order`, left to right.
    fn shows_order(&self, order: &[usize]) -> bool {
        self.main.iter().zip(order).all(|(bar, &value)| {
            bar.is_some_and(|bar| self.shows[bar] == value)
        })
    }
}

/// Builds the scene, playing each lit bar's note on `instrument`.
fn build_animation(instrument: &Instrument) -> CompiledScene {
    let mut scene = Scene::new();
    scene.set_background(Color::from_rgb8(12, 15, 23));
    // Every move uses these unless it says otherwise. Ease-out starts at
    // full speed: an interrupted slide restarts from rest, so with an
    // ease-in curve a bar that keeps moving would barely move until its
    // last beat, then rush the whole way.
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
    let mut stage = Stage::new(&mut scene, instrument, &order);

    let mut steps = Vec::new();
    for (index, algorithm) in ALGORITHMS.iter().enumerate() {
        if index > 0 {
            rng.shuffle(&mut order);
            stage.arrange(&order);
            steps.push(title.set_content("Shuffling"));
            steps.push(all(stage.sync(SHUFFLE_TIME, &[])));
        }
        steps.push(title.set_content(algorithm.name));

        let mut tracked = Tracked::new(&order);
        (algorithm.sort)(&mut tracked);
        let ops;
        (order, ops) = tracked.finish();
        debug_assert!(
            order.is_sorted(),
            "{} left bars unsorted",
            algorithm.name
        );

        let buffer_row = BUFFER_STYLE == BufferStyle::Row
            && ops.iter().any(|op| matches!(op, Op::Move { .. }));
        if buffer_row {
            stage.split = true;
            steps.push(all(stage.sync(SPLIT_TIME, &[])));
        }
        steps.push(wait(PAUSE_BEFORE));

        let beats: Vec<&[Op]> = ops.chunk_by(acts_on).collect();
        let gap = (SORT_TIME / beats.len().max(1) as u32)
            .clamp(MIN_BEAT, MAX_BEAT);
        let mut lit = Vec::new();
        let beats = beats.into_iter().map(|beat| {
            lit.clear();
            for &op in beat {
                stage.apply(op, &mut lit);
            }
            // Beats come faster than fades and moves finish, so a beat
            // takes over a bar from where the last one left it: a bar lit
            // again mid-fade lights up from there, and a bar moved again
            // mid-slide turns towards its new place. Notes always ring out.
            all(stage.sync(MOVE_TIME, &lit)).interrupt()
        });
        steps.push(flow(gap, beats.collect::<Vec<_>>()));
        debug_assert!(
            stage.shows_order(&order),
            "the bars don't show {}'s result",
            algorithm.name
        );

        steps.push(wait(ms(200)));
        if buffer_row {
            stage.split = false;
            steps.push(all(stage.sync(SPLIT_TIME, &[])));
        }

        // Sorted now, so value order is left-to-right order: sweep across,
        // climbing the scale. The `flow` moves on after 15 ms while each
        // fade and note carries on.
        steps.push(flow(
            ms(15),
            stage.main.iter().map(|bar| {
                let bar =
                    bar.expect("every bar is back in the main row");
                let (handle, value) =
                    (stage.bars[bar], stage.shows[bar]);
                all([
                    chain([
                        handle.set_fill(WHITE),
                        handle
                            .fill_to(bar_color(value))
                            .over(FADE_TIME),
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
