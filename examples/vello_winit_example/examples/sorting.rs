//! Sorting algorithms: `BAR_COUNT` bars, coloured by height, start shuffled
//! and are sorted by each algorithm in turn, reshuffling in between.
//!
//! The file reads top to bottom in three parts:
//!
//! 1. **The algorithms.** Each is ordinary Rust that sorts [`Bars`]
//!    through its methods: comparing, swapping and reading values, and
//!    moving them to and from a scratch buffer. They know nothing about
//!    animation.
//! 2. **Layout.** Small functions that turn a slot or a value into a
//!    position, height or colour.
//! 3. **The scene.** Each [`Bars`] method animates what it does as it
//!    does it, as a Motion Canvas or Manim scene would: compared and read
//!    bars light up and play a note whose pitch follows their height,
//!    swapped bars slide or exchange heights ([`SWAP_STYLE`]), and moved
//!    bars drop into a buffer row below the main one or, with the buffer
//!    hidden, overwrite the bar in their slot ([`BUFFER_STYLE`]). A
//!    comparison or read starts a beat, and the swap or move it decides
//!    happens in the same beat. Beats start `BEAT` apart without waiting
//!    for slides or fades; a bar that moves again mid-slide turns towards
//!    its new place from wherever it is.
//!
//! The sorts run once, while the scene is built. Playback only samples the
//! finished timeline, which is why scrubbing backwards works. Sound plays
//! only while time moves forwards.
//!
//! To add an algorithm, write a function with the same signature as
//! [`bubble_sort`] and add it to [`ALGORITHMS`].

use crate::Loc::{Buffer, Main};
use motiongfx::prelude::{ease, ms};
use motiongfx_2d::{
    Animation, Color, CompiledScene, Handle, Point, Rectangle, Scene,
    Signal, Text, all, anchor, chain,
    colors::{WHITE, hsl},
    computed, flow, signal, wait,
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

/// Time between beats. A beat is one comparison or read, with the swap or
/// move it decided, so this sets how long each sort takes.
const BEAT: Duration = ms(20);
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
    sort: fn(&mut Bars),
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

/// Sweeps adjacent pairs, swapping those out of order; each sweep carries
/// the largest remaining value to the end. Stops after a sweep with no
/// swaps.
fn bubble_sort(a: &mut Bars) {
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
fn selection_sort(a: &mut Bars) {
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
fn insertion_sort(a: &mut Bars) {
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
fn comb_sort(a: &mut Bars) {
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
fn quick_sort(a: &mut Bars) {
    let len = a.len();
    quick_sort_range(a, 0, len);
}

/// Sorts main slots `lo..hi` and leaves the rest alone.
fn quick_sort_range(a: &mut Bars, lo: usize, hi: usize) {
    if hi - lo <= 1 {
        return;
    }
    let pivot = partition(a, lo, hi);
    quick_sort_range(a, lo, pivot);
    quick_sort_range(a, pivot + 1, hi);
}

/// Splits main slots `lo..hi` around the last one's value and returns the
/// pivot's final slot.
fn partition(a: &mut Bars, lo: usize, hi: usize) -> usize {
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
fn bitonic_sort(a: &mut Bars) {
    let len = a.len();
    bitonic_sort_range(a, 0, len, true);
}

/// Sorts main slots `lo..hi` ascending or descending and leaves the rest
/// alone.
fn bitonic_sort_range(
    a: &mut Bars,
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
    a: &mut Bars,
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
fn max_value(a: &mut Bars) -> Option<usize> {
    let len = a.len();
    (0..len).map(|slot| a.read(Main(slot))).max()
}

/// Sorts by binary digit, starting from the highest bit any value uses:
/// values with that bit clear go left, values with it set go right, and
/// each side is then sorted by the next bit down. It never compares two
/// values, only reads their bits.
fn radix_msd_sort(a: &mut Bars) {
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
fn radix_msd_range(a: &mut Bars, lo: usize, hi: usize, bit: u32) {
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

/// Sorts each half, then merges the two sorted halves: it repeatedly moves
/// the smaller of their first values into the buffer, then moves the
/// merged run back. Equal values keep their order, so the sort is stable.
fn merge_sort(a: &mut Bars) {
    let len = a.len();
    merge_sort_range(a, 0, len);
}

/// Sorts main slots `lo..hi` and leaves the rest alone.
fn merge_sort_range(a: &mut Bars, lo: usize, hi: usize) {
    if hi - lo <= 1 {
        return;
    }
    let mid = lo + (hi - lo) / 2;
    merge_sort_range(a, lo, mid);
    merge_sort_range(a, mid, hi);
    // The first value not yet merged from each half.
    let mut left = lo;
    let mut right = mid;
    for slot in lo..hi {
        // Taking the left value on a tie keeps the sort stable.
        let take_left =
            right == hi || (left < mid && !a.less(right, left));
        let from = if take_left { &mut left } else { &mut right };
        a.move_value(Main(*from), Buffer(slot));
        *from += 1;
    }
    for slot in lo..hi {
        a.move_value(Buffer(slot), Main(slot));
    }
}

/// Counts how many times each value appears, which gives each value's
/// first slot in the sorted order, then moves every value, in order, into
/// the next free slot for it. It never compares two values; it needs one
/// counter per value up to the largest.
fn counting_sort(a: &mut Bars) {
    sort_by_key_counts(a, |value| value);
}

/// Counting sort on one base-4 digit at a time, from the lowest. Each pass
/// fills four buckets, one per digit, side by side in the row.
/// Because each pass is stable, values with the same digit stay in the
/// order the lower digits set, so after the highest digit's pass the row
/// is sorted.
fn radix_lsd_sort(a: &mut Bars) {
    const RADIX: usize = 4;
    // The place value of the digit this pass sorts by: 1, 4, 16, ...
    let mut place = 1;
    loop {
        // Each pass reads every value anyway, so it also finds the largest
        // instead of a separate sweep before the first pass.
        let max =
            sort_by_key_counts(a, |value| value / place % RADIX);
        match place.checked_mul(RADIX) {
            Some(next) if next <= max => place = next,
            // `max` has no digit above this one.
            _ => break,
        }
    }
}

/// Rearranges the main row so keys ascend, keeping values with equal keys
/// in their current order, and returns the largest value (0 if there are
/// none).
///
/// Counts each key while moving every value into the buffer, which gives
/// each key's first slot. Then it moves every value back, in order, into
/// its key's next free slot.
fn sort_by_key_counts(
    a: &mut Bars,
    key: impl Fn(usize) -> usize,
) -> usize {
    // One counter per key up to the largest seen so far, so the counting
    // sweep needs no earlier sweep to find the largest key.
    let mut next_slot = Vec::new();
    let mut max = 0;
    for slot in 0..a.len() {
        let value = a.read(Main(slot));
        max = max.max(value);
        let value_key = key(value);
        if value_key >= next_slot.len() {
            next_slot.resize(value_key + 1, 0);
        }
        next_slot[value_key] += 1;
        a.move_value(Main(slot), Buffer(slot));
    }
    // Each key's first slot comes after every smaller key's values.
    let mut start = 0;
    for count in &mut next_slot {
        let key_total = *count;
        *count = start;
        start += key_total;
    }
    for slot in 0..a.len() {
        let bucket = &mut next_slot[key(a.read(Buffer(slot)))];
        a.move_value(Buffer(slot), Main(*bucket));
        *bucket += 1;
    }
    max
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
    /// The middle of `slot`'s bottom edge, where its bar is anchored.
    fn base(self, slot: usize) -> Point {
        Point::new(slot_x(slot), self.baseline)
    }

    /// Height of the bar for `value` (`0..BAR_COUNT`); the smallest is
    /// still visible.
    fn height(self, value: usize) -> f64 {
        (value + 1) as f64 / BAR_COUNT as f64 * self.max_height
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

/// A place that holds one value: a slot of the main row, or of the scratch
/// buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Loc {
    Main(usize),
    Buffer(usize),
}

/// One bar on screen.
struct Bar {
    rect: Handle<Rectangle>,
    /// The bar's own colour, which follows the value it shows.
    color: Signal<Color>,
    /// How brightly it is lit: 0 shows its colour, 1 white. Lighting is
    /// its own signal, so a flash and a change of colour never animate the
    /// same property and can run at once.
    glow: Signal<f64>,
}

/// The values being sorted and the bars that show them. Sorts call
/// [`less`](Self::less), [`read`](Self::read), [`swap`](Self::swap) and
/// [`move_value`](Self::move_value), and each call animates what it does.
struct Bars<'a> {
    instrument: &'a Instrument,
    bars: Vec<Bar>,
    /// The value in each main-row slot; `None` while it is in the buffer.
    main: Vec<Option<usize>>,
    /// The scratch buffer, as long as the main row and empty between sorts.
    buffer: Vec<Option<usize>>,
    /// Where each bar is. With a hidden buffer, bars never leave the main
    /// row.
    at: Vec<Loc>,
    /// The value each bar shows. Bars exchange values rather than places
    /// with `SwapStyle::Heights` or a hidden buffer.
    shows: Vec<usize>,
    /// Whether the buffer row is open, shrinking the main row.
    split: bool,
    /// The current sort's animation: finished parts, then its beats since.
    steps: Vec<Animation>,
    beats: Vec<Vec<Animation>>,
    /// The bars lit in the current beat, and whether it has its swap or
    /// move yet.
    lit: Vec<usize>,
    acted: bool,
}

impl<'a> Bars<'a> {
    /// Adds a bar to `scene` for each value in `order`, left to right.
    fn new(
        scene: &mut Scene,
        instrument: &'a Instrument,
        order: &[usize],
    ) -> Self {
        let bars = order
            .iter()
            .enumerate()
            .map(|(slot, &value)| {
                let color = signal(bar_color(value));
                let glow = signal(0.0);
                // Anchored at the bottom, so a bar's position depends only
                // on its slot and its height grows up from the baseline.
                let rect = scene.add(
                    Rectangle::new(BAR_WIDTH, FULL_ROW.height(value))
                        .anchor(anchor::BOTTOM)
                        .position(FULL_ROW.base(slot))
                        .fill(computed!((color, glow) => {
                            color.lerp_rect(WHITE, glow as f32)
                        })),
                );
                Bar { rect, color, glow }
            })
            .collect();
        Self {
            instrument,
            bars,
            main: order.iter().copied().map(Some).collect(),
            buffer: vec![None; order.len()],
            at: (0..order.len()).map(Main).collect(),
            shows: order.to_vec(),
            split: false,
            steps: Vec::new(),
            beats: Vec::new(),
            lit: Vec::new(),
            acted: false,
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

    /// The value at `at`.
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
    /// Starts a beat that lights both.
    #[track_caller]
    fn less(&mut self, i: usize, j: usize) -> bool {
        self.start_beat();
        self.light(Main(i));
        self.light(Main(j));
        self.value(Main(i)) < self.value(Main(j))
    }

    /// The value at `at`, to use as a key: its digits, or which counter it
    /// adds to. Starts a beat that lights it.
    #[track_caller]
    fn read(&mut self, at: Loc) -> usize {
        self.start_beat();
        self.light(at);
        self.value(at)
    }

    /// Swaps main slots `i` and `j`: the bars slide to each other's slots,
    /// or exchange heights.
    fn swap(&mut self, i: usize, j: usize) {
        if i == j {
            return;
        }
        self.main.swap(i, j);
        let a = self.bar_at(Main(i)).expect("swapped an empty slot");
        let b = self.bar_at(Main(j)).expect("swapped an empty slot");
        match SWAP_STYLE {
            SwapStyle::Slide => self.at.swap(a, b),
            SwapStyle::Heights => self.shows.swap(a, b),
        }
        self.join_beat();
        self.place(a, MOVE_TIME);
        self.place(b, MOVE_TIME);
    }

    /// Moves the value at `from` to the empty place `to`. The first move of
    /// a sort opens the buffer row.
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
        if BUFFER_STYLE == BufferStyle::Row && !self.split {
            // The comparison or read that chose this move waits for the
            // row to open, so it lights and sounds with its move.
            let deciding =
                if self.acted { None } else { self.beats.pop() };
            self.set_split(true);
            self.beats.extend(deciding);
        }
        self.join_beat();
        match (BUFFER_STYLE, to) {
            (BufferStyle::Row, _) => {
                let bar =
                    self.bar_at(from).expect("moved a missing bar");
                self.at[bar] = to;
                self.place(bar, MOVE_TIME);
                self.light(to);
            }
            // The bar in the slot shows the value moved into it.
            (BufferStyle::Hidden, Main(_)) => {
                let bar = self
                    .bar_at(to)
                    .expect("every main slot has a bar");
                self.shows[bar] = value;
                self.place(bar, MOVE_TIME);
                self.light(to);
            }
            // The bar stays, showing its value until another moves in.
            (BufferStyle::Hidden, Buffer(_)) => self.light(from),
        }
    }

    /// The bar drawn at `at`, if any.
    fn bar_at(&self, at: Loc) -> Option<usize> {
        self.at.iter().position(|&place| place == at)
    }

    /// Starts a new beat, `BEAT` after the last.
    fn start_beat(&mut self) {
        self.beats.push(Vec::new());
        self.lit.clear();
        self.acted = false;
    }

    /// Adds a swap or move to the current beat, or starts one if it already
    /// has its swap or move.
    fn join_beat(&mut self) {
        if self.beats.is_empty() || self.acted {
            self.start_beat();
        }
        self.acted = true;
    }

    /// Adds `animation` to the current beat.
    fn animate(&mut self, animation: Animation) {
        self.beats
            .last_mut()
            .expect("animations belong to a beat")
            .push(animation);
    }

    /// Animates `bar` to its place and the value it shows, over
    /// `duration`.
    fn moved_to_place(
        &self,
        bar: usize,
        duration: Duration,
    ) -> Animation {
        let (row, slot) = match self.at[bar] {
            Main(slot) if self.split => (MAIN_ROW, slot),
            Main(slot) => (FULL_ROW, slot),
            Buffer(slot) => (BUFFER_ROW, slot),
        };
        let value = self.shows[bar];
        let Bar { rect, color, .. } = &self.bars[bar];
        all([
            rect.position().to(row.base(slot)).over(duration),
            rect.height().to(row.height(value)).over(duration),
            color.to(bar_color(value)).over(duration),
        ])
    }

    /// Animates `bar` to its place in the current beat.
    fn place(&mut self, bar: usize, duration: Duration) {
        let animation = self.moved_to_place(bar, duration);
        self.animate(animation);
    }

    /// Lights the bar at `at`, if there is one, once per beat.
    fn light(&mut self, at: Loc) {
        let Some(bar) = self.bar_at(at) else {
            return;
        };
        if !self.lit.contains(&bar) {
            self.lit.push(bar);
            let animation = self.flash(bar, LIGHT_TIME);
            self.animate(animation);
        }
    }

    /// Turns `bar` white over `light`, fades it back and plays the note of
    /// the value it shows.
    fn flash(&self, bar: usize, light: Duration) -> Animation {
        let glow = &self.bars[bar].glow;
        all([
            chain([
                glow.to(1.0).over(light),
                glow.to(0.0).over(FADE_TIME),
            ]),
            self.instrument
                .note(bar_note(self.shows[bar]))
                .over(NOTE_TIME),
        ])
    }

    /// Ends the beats so far, so what follows waits for them.
    fn end_beats(&mut self) {
        if self.beats.is_empty() {
            return;
        }
        // Beats come faster than fades and moves finish, so a beat takes
        // over a bar from where the last one left it: a bar lit again
        // mid-fade lights up from there, and a bar moved again mid-slide
        // turns towards its new place. Notes always ring out.
        let beats = std::mem::take(&mut self.beats)
            .into_iter()
            .map(|beat| all(beat).interrupt());
        self.steps.push(flow(BEAT, beats));
    }

    /// Opens or closes the buffer row below the main row.
    fn set_split(&mut self, split: bool) {
        self.end_beats();
        self.split = split;
        let moves = (0..self.bars.len())
            .map(|bar| self.moved_to_place(bar, SPLIT_TIME));
        let moves = all(moves.collect::<Vec<_>>());
        self.steps.push(moves);
    }

    /// The animation of the sort just run, closing the buffer row if it
    /// opened.
    fn take_sort(&mut self) -> Animation {
        self.end_beats();
        if self.split {
            self.set_split(false);
        }
        chain(std::mem::take(&mut self.steps))
    }

    /// Slides the bars into `order`, left to right, for the next sort.
    fn shuffle(&mut self, order: &[usize]) -> Animation {
        let mut slot_of = vec![0; order.len()];
        for (slot, &value) in order.iter().enumerate() {
            slot_of[value] = slot;
            self.main[slot] = Some(value);
        }
        for (at, &value) in self.at.iter_mut().zip(&self.shows) {
            *at = Main(slot_of[value]);
        }
        all((0..self.bars.len())
            .map(|bar| self.moved_to_place(bar, SHUFFLE_TIME))
            .collect::<Vec<_>>())
    }

    /// Flashes the bars left to right, 15 ms apart: once sorted, a climb up
    /// the scale. Each fade and note carries on after the next starts.
    fn sweep(&self) -> Animation {
        flow(
            ms(15),
            (0..self.len()).map(|slot| {
                let bar = self
                    .bar_at(Main(slot))
                    .expect("every slot has a bar");
                self.flash(bar, Duration::ZERO)
            }),
        )
    }

    /// Whether the values are sorted and every bar shows the value in its
    /// slot.
    fn is_sorted(&self) -> bool {
        self.main.is_sorted()
            && self.main.iter().all(Option::is_some)
            && self.at.iter().zip(&self.shows).all(|(&at, &value)| {
                matches!(at, Main(slot) if self.main[slot] == Some(value))
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
    let mut bars = Bars::new(&mut scene, instrument, &order);

    let mut steps = Vec::new();
    for (index, algorithm) in ALGORITHMS.iter().enumerate() {
        if index > 0 {
            rng.shuffle(&mut order);
            steps.push(title.set_content("Shuffling"));
            steps.push(bars.shuffle(&order));
        }
        steps.push(title.set_content(algorithm.name));
        steps.push(wait(PAUSE_BEFORE));
        (algorithm.sort)(&mut bars);
        debug_assert!(
            bars.is_sorted(),
            "{} left bars unsorted",
            algorithm.name
        );
        steps.push(bars.take_sort());
        steps.push(wait(ms(200)));
        steps.push(bars.sweep());
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
