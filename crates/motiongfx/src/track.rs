use core::any::TypeId;
use core::time::Duration;

use alloc::boxed::Box;
use alloc::vec::Vec;
use field_path::field::UntypedField;
use hashbrown::{HashMap, HashSet};
use nonempty::NonEmpty;

use crate::action::{
    ActionClip, ActionId, ActionKey, UntypedSubjectId, paths_alias,
};
use crate::sequence::Sequence;
#[cfg(feature = "diagnostics")]
use crate::time::Range;

/// Identifies a [cue](TrackFragment::cue): a point in time that the
/// combinators shift along with the clips around it. Core gives it no
/// meaning; whoever creates cues chooses the IDs and maps them to their
/// own data (the authoring layer uses them for markers and events).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct CueId(pub u32);

pub trait TrackOrdering {
    /// Run all [`TrackFragment`]s one after another.
    fn ord_chain(self) -> TrackFragment;
    fn ord_all(self) -> TrackFragment;
    fn ord_any(self) -> TrackFragment;
    fn ord_flow(self, delay: Duration) -> TrackFragment;
}

impl<T> TrackOrdering for T
where
    T: IntoIterator<Item = TrackFragment>,
{
    fn ord_chain(self) -> TrackFragment {
        chain(self)
    }

    fn ord_all(self) -> TrackFragment {
        all(self)
    }

    fn ord_any(self) -> TrackFragment {
        any(self)
    }

    fn ord_flow(self, delay: Duration) -> TrackFragment {
        flow(delay, self)
    }
}

/// Run all [`TrackFragment`]s one after another.
#[must_use = "This function consumes all the given tracks and returns a modified one."]
pub fn chain(
    tracks: impl IntoIterator<Item = TrackFragment>,
) -> TrackFragment {
    let mut tracks_iter = tracks.into_iter();
    let mut track = tracks_iter.next().unwrap_or_default();

    let mut chain_duration = track.duration;

    for mut other_track in tracks_iter {
        for (key, mut other_sequence) in other_track.sequences.drain()
        {
            other_sequence.delay(chain_duration);
            track = track.upsert_sequence(key, other_sequence);
        }
        track.take_cues(other_track.cues, chain_duration);
        track.interrupting.append(&mut other_track.interrupting);

        chain_duration =
            chain_duration.saturating_add(other_track.duration);
    }

    track.duration = chain_duration;
    track
}

/// Run all [`Track`]s concurrently and wait for all of them to finish.
#[must_use = "This function consumes all the given tracks and returns a modified one."]
pub fn all(
    tracks: impl IntoIterator<Item = TrackFragment>,
) -> TrackFragment {
    let mut tracks_iter = tracks.into_iter();
    let mut track = tracks_iter.next().unwrap_or_default();

    let mut max_duration = track.duration;

    for mut other_track in tracks_iter {
        max_duration = max_duration.max(other_track.duration);

        for (key, other_sequence) in other_track.sequences.drain() {
            track = track.upsert_sequence(key, other_sequence);
        }
        track.take_cues(other_track.cues, Duration::ZERO);
        track.interrupting.append(&mut other_track.interrupting);
    }

    track.duration = max_duration;
    track
}

/// Run all [`Track`]s concurrently and wait for any of them to finish.
#[must_use = "This function consumes all the given tracks and returns a modified one."]
pub fn any(
    tracks: impl IntoIterator<Item = TrackFragment>,
) -> TrackFragment {
    let mut tracks_iter = tracks.into_iter();
    let mut track = tracks_iter.next().unwrap_or_default();

    let mut min_duration = track.duration;

    for mut other_track in tracks_iter {
        min_duration = min_duration.min(other_track.duration);

        for (key, other_sequence) in other_track.sequences.drain() {
            track = track.upsert_sequence(key, other_sequence);
        }
        track.take_cues(other_track.cues, Duration::ZERO);
        track.interrupting.append(&mut other_track.interrupting);
    }

    track.duration = min_duration;
    track
}

/// Run one [`Track`] after another with a fixed delay time.
#[must_use = "This function consumes all the given tracks and returns a modified one."]
pub fn flow(
    delay: Duration,
    tracks: impl IntoIterator<Item = TrackFragment>,
) -> TrackFragment {
    let mut tracks_iter = tracks.into_iter();
    let mut track = tracks_iter.next().unwrap_or_default();

    let mut flow_delay = Duration::ZERO;
    let mut final_duration = track.duration;

    for other_track in tracks_iter {
        flow_delay = flow_delay.saturating_add(delay);
        final_duration = flow_delay
            .saturating_add(other_track.duration)
            .max(final_duration);

        for (key, mut sequence) in other_track.sequences {
            sequence.delay(flow_delay);
            track = track.upsert_sequence(key, sequence);
        }
        track.take_cues(other_track.cues, flow_delay);
        track.interrupting.extend(other_track.interrupting);
    }

    track.duration = final_duration;
    track
}

/// Run a [`Track`] after a fixed delay time.
#[must_use = "This function consumes the given track and returns a modified one."]
pub fn delay(
    delay: Duration,
    mut track: TrackFragment,
) -> TrackFragment {
    for sequence in track.sequences.values_mut() {
        sequence.delay(delay);
    }
    for (_, time) in &mut track.cues {
        *time = time.saturating_add(delay);
    }

    track.duration = track.duration.saturating_add(delay);
    track
}

pub struct TrackFragment {
    sequences: HashMap<ActionKey, Sequence>,
    duration: Duration,
    /// Cued points in time, relative to this fragment's start. Shifted
    /// by the combinators exactly like clips. Empty (no allocation) in
    /// fragments without cues.
    cues: Vec<(CueId, Duration)>,
    /// Clips that cut a running clip on the same field instead of
    /// conflicting with it, each with the group it was marked in (the
    /// group's smallest clip ID); see [`Self::interrupting`]. Empty (no
    /// allocation) in fragments that don't interrupt.
    interrupting: Vec<(ActionId, ActionId)>,
}

impl TrackFragment {
    /// Duration reserved by this fragment's actions and ordering combinators.
    /// Useful for locating a chapter boundary before appending the fragment
    /// to a larger sequence. Final compilation can extend the span when
    /// resolving conflicting actions on the same field.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn new() -> Self {
        Self {
            sequences: HashMap::new(),
            duration: Duration::ZERO,
            cues: Vec::new(),
            interrupting: Vec::new(),
        }
    }

    pub fn single(key: ActionKey, clip: ActionClip) -> Self {
        Self {
            duration: clip.duration,
            sequences: [(key, Sequence::new(clip))].into(),
            cues: Vec::new(),
            interrupting: Vec::new(),
        }
    }

    /// A zero-length fragment that cues its start time. Combinators
    /// place it like any other fragment: in a chain, where the previous
    /// fragment ends; in `all`/`any`, at the group start; in `flow`, at
    /// its own slot.
    pub fn cue(id: CueId) -> Self {
        Self {
            sequences: HashMap::new(),
            duration: Duration::ZERO,
            cues: alloc::vec![(id, Duration::ZERO)],
            interrupting: Vec::new(),
        }
    }

    /// Cue the point `time` after this fragment's start.
    pub fn add_cue(&mut self, id: CueId, time: Duration) {
        self.cues.push((id, time));
    }

    /// Cued points in time, relative to this fragment's start.
    pub fn cues(&self) -> &[(CueId, Duration)] {
        &self.cues
    }

    /// Append `cues`, shifted `offset` later.
    fn take_cues(
        &mut self,
        cues: Vec<(CueId, Duration)>,
        offset: Duration,
    ) {
        self.cues.extend(
            cues.into_iter()
                .map(|(id, time)| (id, time.saturating_add(offset))),
        );
    }

    /// A fragment with no clips of its own, reserving `duration` of
    /// time - for a slot in the tree nothing resolves into an action
    /// yet.
    pub fn silent(duration: Duration) -> Self {
        Self {
            sequences: HashMap::new(),
            duration,
            cues: Vec::new(),
            interrupting: Vec::new(),
        }
    }

    /// Lets every clip in this fragment interrupt a clip on the same
    /// field that is still running when it starts, instead of
    /// conflicting with it.
    ///
    /// The running clip is cut where this one starts: up to the cut it
    /// follows its original timing, and the interrupting clip starts
    /// from the value it had reached. The fragment's clips form one
    /// group: a clip of an interrupting group that started earlier, which
    /// would start while this one runs (such as the rest of the chain
    /// this one cut), is dropped, since this fragment has taken over the
    /// field by then. Otherwise a clip starting at the same instant, or
    /// on an aliasing field path, still conflicts.
    ///
    /// Marking a fragment again makes all its clips one group.
    #[must_use = "This method consumes the fragment and returns a marked one."]
    pub fn interrupting(mut self) -> Self {
        let group = self
            .sequences
            .values()
            .flat_map(|sequence| sequence.clips.iter())
            .map(|clip| clip.id)
            .min();
        let Some(group) = group else {
            return self;
        };
        self.interrupting = self
            .sequences
            .values()
            .flat_map(|sequence| sequence.clips.iter())
            .map(|clip| (clip.id, group))
            .collect();
        self
    }

    /// Updates or inserts a [`Sequence`] in a track.
    ///
    /// If the [`ActionKey`] already exists, this method appends the
    /// clips of the `new_sequence` to the existing sequence.
    /// If the [`ActionKey`] does not exist, a new entry is created
    /// for the `new_sequence`.
    ///
    /// This method consumes `self` and returns a modified instance,
    /// following a builder pattern.
    ///
    /// # Parameters
    ///
    /// * `key`: The unique identifier for the track.
    /// * `new_sequence`: The sequence to be added or extended.
    pub fn upsert_sequence(
        mut self,
        key: ActionKey,
        new_sequence: Sequence,
    ) -> Self {
        match self.sequences.get_mut(&key) {
            Some(sequence) => {
                sequence.extend(new_sequence);
            }
            None => {
                self.sequences.insert(key, new_sequence);
            }
        }

        self
    }

    pub fn compile(self) -> Track {
        let mut sequences =
            self.sequences.into_iter().collect::<Vec<_>>();

        // Before anything reads a sequence's first/last clip as its
        // start/end, and before cutting, which compares neighbours.
        // Conflict resolution picks winners by `ActionId`, so it does
        // not depend on this order.
        for (_, sequence) in sequences.iter_mut() {
            sequence.sort_by_start();
        }

        // Cut first, so a clip an interrupting clip takes over from no
        // longer overlaps it and is not reported as a conflict.
        let cuts = cut_interrupted(&mut sequences, self.interrupting);

        #[cfg(feature = "diagnostics")]
        let conflicts = resolve_conflicts(&mut sequences);
        #[cfg(not(feature = "diagnostics"))]
        resolve_conflicts(&mut sequences);

        // Stable, so cues at the same time keep their authoring order.
        let mut cues = self.cues;
        cues.sort_by_key(|(_, time)| *time);
        let cues = cues.into_boxed_slice();
        let last_cue = cues.last().map(|(_, time)| *time);

        if sequences.is_empty() {
            return Track {
                field_lookups: Box::new([]),
                sequence_spans: Box::new([]),
                clip_arena: Box::new([]),
                cut_progress: Box::new([]),
                bake_order: Box::new([]),
                #[cfg(feature = "diagnostics")]
                conflicts,
                cues,
                duration: self
                    .duration
                    .max(last_cue.unwrap_or_default()),
            };
        }

        sequences.sort_by_key(|(key, _)| *key.field());

        // The combinators accumulate `duration` independently of the
        // clip offsets, so pin them together here: the clamp in
        // `Timeline::set_target_time` must be able to reach the last
        // clip's end.
        let duration = sequences
            .iter()
            .map(|(_, seq)| seq.end())
            .chain(last_cue)
            .max()
            .unwrap_or(Duration::ZERO)
            .max(self.duration);

        let mut seq_offset = 0;
        let mut sequence_spans = Vec::with_capacity(sequences.len());

        let mut field = sequences[0].0.field();
        let mut field_offset = 0;
        let mut field_len = 0;
        let mut field_lookups = Vec::new();

        for (key, seq) in sequences.iter() {
            sequence_spans.push((
                *key,
                Span {
                    offset: seq_offset,
                    len: seq.len(),
                },
            ));
            seq_offset += seq.len();

            if key.field() != field {
                field_lookups.push((
                    *field,
                    Span {
                        offset: field_offset,
                        len: field_len,
                    },
                ));

                field = key.field();
                field_offset += field_len;
                field_len = 0;
            }
            field_len += 1;
        }

        // Final field.
        field_lookups.push((
            *field,
            Span {
                offset: field_offset,
                len: field_len,
            },
        ));

        let clip_arena = sequences
            .into_iter()
            .flat_map(|(_, clips)| clips)
            .collect::<Box<[_]>>();

        let cut_progress = cut_progress(&clip_arena, cuts);

        // `clip_arena` stays grouped by sequence for the per-field
        // spans; `bake_order` is the same clips in start-time order,
        // ties by `ActionId`.
        let mut bake_order =
            (0..clip_arena.len() as u32).collect::<Box<[_]>>();
        bake_order.sort_unstable_by_key(|&i| {
            let clip = &clip_arena[i as usize];
            (clip.start, clip.id)
        });

        Track {
            field_lookups: field_lookups.into_boxed_slice(),
            sequence_spans: sequence_spans.into_boxed_slice(),
            clip_arena,
            cut_progress,
            bake_order,
            #[cfg(feature = "diagnostics")]
            conflicts,
            cues,
            duration,
        }
    }
}

impl Default for TrackFragment {
    fn default() -> Self {
        Self::new()
    }
}

/// A clip dropped when the track compiled because a later action on
/// an aliasing field path of the same subject overlapped it.
#[cfg(feature = "diagnostics")]
#[derive(Debug, Clone, Copy)]
pub struct FieldConflict {
    pub dropped: ActionId,
    pub dropped_field: UntypedField,
    /// Time span of the dropped clip.
    pub dropped_span: Range,
    pub winner: ActionId,
    pub winner_field: UntypedField,
    /// Where the two clips overlapped.
    pub overlap: Range,
}

#[cfg(feature = "diagnostics")]
type ConflictReport = Box<[FieldConflict]>;
#[cfg(not(feature = "diagnostics"))]
type ConflictReport = ();

/// Cuts every clip that the next clip on the same field interrupts,
/// returning each cut clip's ID with the fraction of its original
/// duration it keeps.
///
/// Sequences must be sorted by start. First, clips an interrupting group
/// took over are dropped; see [`drop_taken_over`]. Then a clip is cut
/// when the next one is marked interrupting and starts strictly inside
/// it. Clips that start together, or that still overlap after cutting,
/// are left for `resolve_conflicts`.
fn cut_interrupted(
    sequences: &mut [(ActionKey, Sequence)],
    mut interrupting: Vec<(ActionId, ActionId)>,
) -> Vec<(ActionId, f32)> {
    let mut cuts = Vec::new();
    if interrupting.is_empty() {
        return cuts;
    }
    interrupting.sort_unstable();

    let group_of = |id: ActionId| {
        interrupting
            .binary_search_by_key(&id, |&(clip, _)| clip)
            .ok()
            .map(|found| interrupting[found].1)
    };
    // When each group starts: its earliest clip's start.
    let mut group_starts = sequences
        .iter()
        .flat_map(|(_, sequence)| sequence.clips.iter())
        .filter_map(|clip| Some((group_of(clip.id)?, clip.start)))
        .collect::<Vec<_>>();
    // Sorted by group, then start, so dedup keeps each group's earliest.
    group_starts.sort_unstable();
    group_starts.dedup_by_key(|&mut (group, _)| group);
    let started = |id: ActionId| {
        let group = group_of(id)?;
        group_starts
            .binary_search_by_key(&group, |&(group, _)| group)
            .ok()
            .map(|found| group_starts[found].1)
    };

    // Scratch buffers reused across sequences.
    let mut scratch = Scratch::default();
    for (_, sequence) in sequences.iter_mut() {
        drop_taken_over(sequence, &started, &mut scratch);
        for index in 1..sequence.len() {
            let clip = sequence.clips[index];
            if group_of(clip.id).is_none() {
                continue;
            }
            let previous = &mut sequence.clips[index - 1];
            if previous.start < clip.start
                && clip.start < previous.end()
            {
                let kept = clip.start - previous.start;
                let progress = (kept.as_secs_f64()
                    / previous.duration.as_secs_f64())
                    as f32;
                previous.duration = kept;
                cuts.push((previous.id, progress));
            }
        }
    }
    cuts
}

/// Drops each clip of an interrupting group that starts while (or as) a
/// clip of a later-starting interrupting group runs. That group has
/// taken over the field by then, so the rest of the earlier one no longer
/// applies: a new "fade in, then out" that starts mid-way through an
/// earlier one replaces the earlier fade-out instead of conflicting with
/// it.
///
/// `started` gives an interrupting clip's group start, or `None` for a
/// clip that doesn't interrupt, which is never dropped. `sequence` must
/// be sorted by start. Clips of the latest-starting group are never
/// dropped, so the sequence stays non-empty.
fn drop_taken_over(
    sequence: &mut Sequence,
    started: &impl Fn(ActionId) -> Option<Duration>,
    scratch: &mut Scratch,
) {
    let Scratch {
        clips,
        running,
        dropped,
    } = scratch;
    clips.clear();
    clips.extend(sequence.clips.iter().copied());
    running.clear();
    dropped.clear();
    // Clips starting together all count as running before any of them is
    // checked, so a group taking over at that instant is seen.
    for together in clips.chunk_by(|a, b| a.start == b.start) {
        let start = together[0].start;
        running.retain(|(earlier, _)| earlier.end() > start);
        running.extend(
            together
                .iter()
                .filter(|clip| clip.duration > Duration::ZERO)
                .filter_map(|&clip| Some((clip, started(clip.id)?))),
        );
        for clip in together {
            let Some(group_start) = started(clip.id) else {
                continue;
            };
            if running.iter().any(|&(_, other)| group_start < other) {
                dropped.push(clip.id);
                running.retain(|(running, _)| running.id != clip.id);
            }
        }
    }
    if dropped.is_empty() {
        return;
    }
    dropped.sort_unstable();
    let kept = clips
        .iter()
        .copied()
        .filter(|clip| dropped.binary_search(&clip.id).is_err())
        .collect();
    sequence.clips = NonEmpty::from_vec(kept).expect(
        "clips of the latest-starting group are never dropped",
    );
}

/// Buffers [`drop_taken_over`] reuses across sequences: the sequence's
/// clips as a slice, the interrupting clips still running with their
/// group's start, and the clips dropped.
#[derive(Default)]
struct Scratch {
    clips: Vec<ActionClip>,
    running: Vec<(ActionClip, Duration)>,
    dropped: Vec<ActionId>,
}

/// The kept fraction of every clip in `clip_arena`, by index: 1 for
/// clips that run in full. Empty, and so free to look up, when nothing
/// was cut.
fn cut_progress(
    clip_arena: &[ActionClip],
    mut cuts: Vec<(ActionId, f32)>,
) -> Box<[f32]> {
    if cuts.is_empty() {
        return Box::new([]);
    }
    cuts.sort_unstable_by_key(|&(id, _)| id);
    clip_arena
        .iter()
        .map(|clip| {
            cuts.binary_search_by_key(&clip.id, |&(id, _)| id)
                .map_or(1.0, |found| cuts[found].1)
        })
        .collect()
}

/// Only fields of the same subject and source type can alias, so
/// conflicts are searched within these groups only.
fn conflict_group(key: &ActionKey) -> (UntypedSubjectId, TypeId) {
    (*key.subject_id(), key.field().source_id())
}

/// Drops any clip that a later-authored action on an aliasing field
/// path of the same subject overlaps in time. The later action wins
/// the whole clip, not just the overlapped span.
///
/// Two clips overlap when `a.start < b.end() && b.start < a.end()`, so
/// touching clips and zero-length clips at another clip's start do not.
///
/// Sequences are grouped by subject and source type, then each group's
/// clips are swept in start order against the clips still running.
/// Cost grows with the number of clips plus the pairs that run at the
/// same time within one subject, not with every pair of fields.
fn resolve_conflicts(
    sequences: &mut Vec<(ActionKey, Sequence)>,
) -> ConflictReport {
    let mut removed = HashSet::<ActionId>::new();
    #[cfg(feature = "diagnostics")]
    let mut conflicts = Vec::new();

    let mut order = (0..sequences.len()).collect::<Vec<_>>();
    order.sort_unstable_by_key(|&index| {
        conflict_group(&sequences[index].0)
    });

    // Scratch buffers reused across groups: every clip of the group in
    // start order, and the clips still running at the sweep position.
    // Each clip is paired with the index of its sequence.
    let mut clips = Vec::<(ActionClip, usize)>::new();
    let mut running = Vec::<(ActionClip, usize)>::new();

    for group in order.chunk_by(|&a, &b| {
        conflict_group(&sequences[a].0)
            == conflict_group(&sequences[b].0)
    }) {
        clips.clear();
        for &index in group {
            clips.extend(
                sequences[index]
                    .1
                    .clips
                    .iter()
                    .map(|&clip| (clip, index)),
            );
        }
        clips.sort_unstable_by_key(|(clip, _)| (clip.start, clip.id));

        running.clear();
        for &(clip, index) in &clips {
            // A clip that ended by this start cannot overlap this clip
            // or any later one, which all start at or after it.
            running.retain(|(earlier, _)| earlier.end() > clip.start);

            for &(earlier, earlier_index) in &running {
                // `earlier.start <= clip.start < earlier.end()` holds
                // here; the overlap also needs the reverse bound, which
                // only fails for a zero-length clip at earlier's start.
                if earlier.start >= clip.end() {
                    continue;
                }
                let earlier_key = &sequences[earlier_index].0;
                let key = &sequences[index].0;
                if !paths_alias(
                    earlier_key.field().field_path(),
                    key.field().field_path(),
                ) {
                    continue;
                }

                // The later-authored action (higher id) wins. The
                // winner and keys are only read by diagnostics/tracing.
                #[cfg_attr(
                    not(all(
                        feature = "diagnostics",
                        feature = "tracing"
                    )),
                    allow(unused_variables)
                )]
                let ((dropped, dropped_key), (winner, winner_key)) =
                    if earlier.id < clip.id {
                        ((earlier, earlier_key), (clip, key))
                    } else {
                        ((clip, key), (earlier, earlier_key))
                    };
                if !removed.insert(dropped.id) {
                    continue;
                }

                #[cfg(feature = "diagnostics")]
                conflicts.push(FieldConflict {
                    dropped: dropped.id,
                    dropped_field: *dropped_key.field(),
                    dropped_span: Range {
                        start: dropped.start,
                        end: dropped.end(),
                    },
                    winner: winner.id,
                    winner_field: *winner_key.field(),
                    overlap: Range {
                        start: dropped.start.max(winner.start),
                        end: dropped.end().min(winner.end()),
                    },
                });
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    "dropping action on `{}` ({:?}..{:?}): a later action on `{}` overlaps it",
                    dropped_key.field().field_path(),
                    dropped.start,
                    dropped.end(),
                    winner_key.field().field_path(),
                );
            }

            running.push((clip, index));
        }
    }

    if !removed.is_empty() {
        sequences.retain_mut(|(_, seq)| {
            let kept = seq
                .clips
                .iter()
                .copied()
                .filter(|clip| !removed.contains(&clip.id))
                .collect::<Vec<_>>();

            match NonEmpty::from_vec(kept) {
                Some(clips) => {
                    seq.clips = clips;
                    true
                }
                None => false,
            }
        });
    }

    #[cfg(feature = "diagnostics")]
    {
        conflicts.into_boxed_slice()
    }
}

/// A compiled dense action sequences, optimized for playback and
/// queries.
///
/// A `Track` is created from a [`TrackFragment`] and provides an
/// immutable, space-efficient layout. [`ActionClip`]s are stored
/// in a flat array with spans for quick access.
#[derive(Debug, Clone)]
pub struct Track {
    // TODO: Use this to optimized baking/sampling? (There are no
    // use case for the lookups atm!)
    /// Lookup from each field to the range of actions affecting it.
    ///
    /// Each entry holds an [`UntypedField`] and a [`Span`] into
    /// `clip_spans`.
    field_lookups: Box<[(UntypedField, Span)]>,

    /// [`ActionClip`]s grouped by [`ActionKey`] in sorted order.
    ///
    /// Each entry holds an [`ActionKey`] and a [`Span`] into
    /// `clip_arena`.
    sequence_spans: Box<[(ActionKey, Span)]>,

    /// Contiguous storage of all action clips.
    clip_arena: Box<[ActionClip]>,

    /// For each clip in `clip_arena`, the fraction of its original
    /// duration it keeps after an interrupting clip cut it (1 if it runs
    /// in full). Empty when no clip in the track was cut.
    cut_progress: Box<[f32]>,

    /// `clip_arena` indices in start-time order, ties by `ActionId`.
    bake_order: Box<[u32]>,

    /// Clips this track dropped to resolve overlapping actions on
    /// aliasing field paths.
    #[cfg(feature = "diagnostics")]
    conflicts: Box<[FieldConflict]>,

    /// Cued points in time, sorted by time.
    cues: Box<[(CueId, Duration)]>,

    /// Total duration of the track.
    ///
    /// Guaranteed to be `>=` the end of every clip in `clip_arena`.
    /// See [`TrackFragment::compile`].
    duration: Duration,
}

impl Track {
    pub fn lookup_field_spans(
        &self,
        field: impl Into<UntypedField>,
    ) -> Option<&[(ActionKey, Span)]> {
        let index = self
            .field_lookups
            .binary_search_by_key(&field.into(), |(f, _)| *f)
            .ok()?;

        let (_, span) = &self.field_lookups[index];

        Some(
            &self.sequence_spans[span.offset..span.offset + span.len],
        )
    }

    #[inline]
    pub fn field_lookups(&self) -> &[(UntypedField, Span)] {
        &self.field_lookups
    }

    #[inline]
    pub fn sequences_spans(&self) -> &[(ActionKey, Span)] {
        &self.sequence_spans
    }

    #[inline]
    pub fn clips(&self, span: Span) -> &[ActionClip] {
        &self.clip_arena[span.offset..span.offset + span.len]
    }

    /// How much of the clip at `index` within `span` remains after an
    /// interrupting clip cut it, as a fraction of its original
    /// duration, or `None` if it runs in full.
    #[inline]
    pub fn cut(&self, span: Span, index: usize) -> Option<f32> {
        self.cut_progress
            .get(span.offset + index)
            .copied()
            .filter(|&progress| progress < 1.0)
    }

    /// Every clip in start-time order, ties broken by `ActionId`.
    #[inline]
    pub fn bake_clips(
        &self,
    ) -> impl Iterator<Item = &ActionClip> + '_ {
        self.bake_order
            .iter()
            .map(|&i| &self.clip_arena[i as usize])
    }

    /// [`Self::bake_clips`], each with the fraction of its clip it keeps
    /// if an interrupting clip cut it.
    pub(crate) fn bake_clips_with_cuts(
        &self,
    ) -> impl Iterator<Item = (&ActionClip, Option<f32>)> + '_ {
        self.bake_order.iter().map(|&i| {
            let i = i as usize;
            let cut = self
                .cut_progress
                .get(i)
                .copied()
                .filter(|&progress| progress < 1.0);
            (&self.clip_arena[i], cut)
        })
    }

    #[inline]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Cued points in time, sorted by time; cues at the same time keep
    /// the order they were added in.
    #[inline]
    pub fn cues(&self) -> &[(CueId, Duration)] {
        &self.cues
    }

    /// Clips this track dropped to resolve overlapping actions on
    /// aliasing field paths.
    #[cfg(feature = "diagnostics")]
    #[inline]
    pub fn conflicts(&self) -> &[FieldConflict] {
        &self.conflicts
    }
}

impl IntoIterator for Track {
    type Item = Self;

    type IntoIter = core::array::IntoIter<Self::Item, 1>;

    fn into_iter(self) -> Self::IntoIter {
        [self].into_iter()
    }
}

/// The [`Track`]s a [`Timeline`](crate::timeline::Timeline) plays
/// through, always at least one.
#[derive(Clone, Debug)]
pub struct TrackList(pub NonEmpty<Track>);

impl From<Track> for TrackList {
    fn from(track: Track) -> Self {
        Self::new(track)
    }
}

impl TrackList {
    pub fn new(track: Track) -> Self {
        Self(NonEmpty::new(track))
    }

    pub fn collect(
        tracks: impl IntoIterator<Item = Track>,
    ) -> Option<Self> {
        NonEmpty::collect(tracks).map(Self)
    }

    pub fn add(
        &mut self,
        tracks: impl IntoIterator<Item = Track>,
    ) -> &mut Self {
        self.0.extend(tracks);
        self
    }

    pub fn into_boxed_slice(self) -> Box<[Track]> {
        self.0.into_iter().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub offset: usize,
    pub len: usize,
}

#[cfg(test)]
mod tests {
    use crate::action::{ActionId, IdRegistry, UntypedSubjectId};
    use crate::time::{cs, ms, s};

    use super::*;

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
    )]
    struct DummyId(u32);

    fn key(path: &'static str) -> ActionKey {
        ActionKey::new(
            UntypedSubjectId::PLACEHOLDER,
            UntypedField::placeholder_with_path(path),
        )
    }

    const fn clip(centis: u64) -> ActionClip {
        ActionClip::new(ActionId::PLACEHOLDER, cs(centis))
    }

    #[test]
    fn track_key_uniqueness() {
        // Sequence with 0 duration to prevent overlaps.
        const DUMMY_SEQ: Sequence = Sequence::new(clip(0));

        let entity1 = DummyId(1);
        let entity2 = DummyId(2);
        let field_u32_a = UntypedField::placeholder_with_path("a");
        let field_u32_b = UntypedField::placeholder_with_path("b");

        let mut id_registry = IdRegistry::new();
        let id1 = id_registry.register_instance(entity1);
        let id2 = id_registry.register_instance(entity2);

        let k1 = ActionKey::new(
            UntypedSubjectId::new::<DummyId>(id1),
            field_u32_a,
        );
        let k2 = ActionKey::new(
            UntypedSubjectId::new::<DummyId>(id2),
            field_u32_a,
        );
        let k3 = ActionKey::new(
            UntypedSubjectId::new::<DummyId>(id1),
            field_u32_b,
        );

        let track = TrackFragment::new()
            .upsert_sequence(k1, DUMMY_SEQ.clone())
            .upsert_sequence(k2, DUMMY_SEQ.clone())
            .upsert_sequence(k3, DUMMY_SEQ.clone())
            // Similar key with the first sequence.
            .upsert_sequence(k1, DUMMY_SEQ.clone());

        assert_eq!(track.sequences.len(), 3);
    }

    #[test]
    fn chain_duration_and_delay() {
        let track1 = TrackFragment::single(key("a"), clip(100));
        let track2 = TrackFragment::single(key("b"), clip(200));

        let track = [track1, track2].ord_chain();

        assert_eq!(track.duration, cs(300));
        let seq_b = &track.sequences[&key("b")];
        // `seq_b` should be delayed by 1.0 (duration of `track1`).
        assert_eq!(seq_b.start(), cs(100));
    }

    #[test]
    fn all_duration_max() {
        let track1 = TrackFragment::single(key("a"), clip(100));
        let track2 = TrackFragment::single(key("b"), clip(300));

        let track = [track1, track2].ord_all();
        assert_eq!(track.duration, cs(300));
    }

    #[test]
    fn any_duration_min() {
        let track1 = TrackFragment::single(key("a"), clip(100));
        let track2 = TrackFragment::single(key("b"), clip(300));

        let track = [track1, track2].ord_any();
        assert_eq!(track.duration, cs(100));
    }

    #[test]
    fn flow_with_delay() {
        let track1 = TrackFragment::single(key("a"), clip(100));
        let track2 = TrackFragment::single(key("b"), clip(100));

        let track = [track1, track2].ord_flow(cs(50));

        // 0.5 delay + 1.0 duration
        assert_eq!(track.duration, cs(150));
        let seq_b = &track.sequences[&key("b")];
        // `seq_b` should be delayed by 0.5
        assert_eq!(seq_b.start(), cs(50));
    }

    const MID: CueId = CueId(0);
    const START: CueId = CueId(1);
    const LATE: CueId = CueId(2);

    #[test]
    fn cues_move_with_their_fragments() {
        let chained = [
            TrackFragment::single(key("a"), clip(100)),
            TrackFragment::cue(MID),
            TrackFragment::single(key("b"), clip(200)),
        ]
        .ord_chain();
        assert_eq!(chained.cues(), [(MID, cs(100))]);
        assert_eq!(chained.duration, cs(300));

        // `all` and `any` place cues at the group start.
        let together = [
            TrackFragment::single(key("a"), clip(100)),
            TrackFragment::cue(START),
        ]
        .ord_all();
        assert_eq!(together.cues(), [(START, cs(0))]);
        assert_eq!(together.duration, cs(100));

        // In `flow` a cue fragment takes its own slot.
        let flowed = [
            TrackFragment::single(key("a"), clip(100)),
            TrackFragment::silent(cs(10)),
            TrackFragment::cue(LATE),
        ]
        .ord_flow(cs(50));
        assert_eq!(flowed.cues(), [(LATE, cs(100))]);

        let delayed = delay(cs(30), TrackFragment::cue(LATE));
        assert_eq!(delayed.cues(), [(LATE, cs(30))]);

        // Nested groups add up their offsets.
        let nested = [
            TrackFragment::silent(cs(20)),
            [TrackFragment::silent(cs(10)), TrackFragment::cue(MID)]
                .ord_chain(),
        ]
        .ord_chain();
        assert_eq!(nested.cues(), [(MID, cs(30))]);
    }

    #[test]
    fn compiled_tracks_sort_cues_and_reach_the_last_one() {
        let mut fragment = TrackFragment::silent(cs(10));
        fragment.add_cue(LATE, cs(50));
        fragment.add_cue(MID, cs(5));
        fragment.add_cue(START, cs(5));

        let track = fragment.compile();
        assert_eq!(
            track.cues(),
            [(MID, cs(5)), (START, cs(5)), (LATE, cs(50)),]
        );
        // The playhead must be able to reach the last cue.
        assert_eq!(track.duration(), cs(50));
    }

    #[test]
    fn delay_applies_offset() {
        let track = TrackFragment::single(key("a"), clip(200));

        let track = delay(cs(150), track);
        let seq_a = &track.sequences[&key("a")];

        assert_eq!(seq_a.start(), cs(150));
        assert_eq!(seq_a.end(), cs(350));
        // The delay is part of the fragment's span.
        assert_eq!(track.duration, cs(350));
    }

    /// `delay` shifts every clip but must widen `duration` to match,
    /// or a combinator consuming the fragment places the next clips
    /// using the understated span and overlaps the delayed ones.
    #[test]
    fn delayed_fragment_chains_without_overlapping() {
        let delayed = delay(
            cs(150),
            TrackFragment::single(key("a"), clip(200)),
        );
        let track =
            [delayed, TrackFragment::single(key("a"), clip(100))]
                .ord_chain();

        assert_eq!(track.duration, cs(450));
        assert_eq!(track.sequences[&key("a")].end(), cs(450));
    }

    /// Chaining durations that have no exact `f32` representation used
    /// to leave `TrackFragment::duration` and the clip offsets on
    /// different values, because the two are accumulated separately.
    /// The mismatch tripped the non-overlap assert in `Sequence::push`
    /// and put `Track::duration` out of reach of the last clip's end.
    #[test]
    fn chain_accumulation_matches_clip_offsets() {
        // 0.1s is not representable in binary floating point.
        let tracks: Vec<_> = (0..10)
            .map(|_| TrackFragment::single(key("a"), clip(10)))
            .collect();

        let track = tracks.ord_chain();

        assert_eq!(track.duration, cs(100));
        assert_eq!(track.sequences[&key("a")].end(), cs(100));
    }

    /// `Track::duration` must always be reachable by the playhead, so
    /// that the final clip can resolve to `SampleMode::End`.
    #[test]
    fn compile_duration_covers_last_clip_end() {
        let mut fragment = TrackFragment::single(key("a"), clip(100));
        // Understate the duration the way a combinator would if the
        // two accumulations ever diverged again.
        fragment.duration = ms(999);

        let track = fragment.compile();

        assert_eq!(track.duration(), cs(100));
    }

    /// Combinator arithmetic has to saturate, or a `Duration::MAX`
    /// duration panics inside `chain`, `flow`, or `ActionClip::end`.
    ///
    /// Distinct keys per fragment: saturated clips really do overlap,
    /// and the non-overlap assert is right to say so. Only the duration
    /// arithmetic is under test.
    #[test]
    fn saturated_durations_do_not_overflow_the_combinators() {
        let huge = |path: &'static str| {
            TrackFragment::single(
                key(path),
                ActionClip::new(ActionId::PLACEHOLDER, Duration::MAX),
            )
        };

        assert_eq!(huge("a").duration, Duration::MAX);
        assert_eq!(
            [huge("a"), huge("b")].ord_chain().duration,
            Duration::MAX
        );
        assert_eq!(
            [huge("a"), huge("b")].ord_flow(s(1)).duration,
            Duration::MAX
        );
        assert_eq!(
            [huge("a"), huge("b")].ord_all().duration,
            Duration::MAX
        );
        assert_eq!(
            delay(Duration::MAX, huge("a")).duration,
            Duration::MAX
        );
    }

    #[test]
    fn compile_empty_fragment_is_not_a_panic() {
        let track = TrackFragment::new().compile();

        assert_eq!(track.duration(), Duration::ZERO);
        assert!(track.sequences_spans().is_empty());
    }

    #[test]
    fn field_spans_cover_every_lane() {
        const DUMMY: Sequence = Sequence::new(clip(0));

        let fa = UntypedField::placeholder_with_path("a");
        let fb = UntypedField::placeholder_with_path("b");
        let fc = UntypedField::placeholder_with_path("c");

        let mut ids = IdRegistry::new();
        let s1 = ids.register_instance(DummyId(1));
        let s2 = ids.register_instance(DummyId(2));

        let k = |sid, field| {
            ActionKey::new(
                UntypedSubjectId::new::<DummyId>(sid),
                field,
            )
        };

        let track = TrackFragment::new()
            .upsert_sequence(k(s1, fa), DUMMY.clone())
            .upsert_sequence(k(s2, fa), DUMMY.clone())
            .upsert_sequence(k(s1, fb), DUMMY.clone())
            .upsert_sequence(k(s1, fc), DUMMY.clone())
            .compile();

        // Fields in the order `compile` sorts them into.
        let mut covered = Vec::new();

        for (field, expected) in [(fa, 2), (fb, 1), (fc, 1)] {
            let spans = track
                .lookup_field_spans(field)
                .expect("field was compiled in");

            assert_eq!(spans.len(), expected, "{field:?}");
            assert!(
                spans.iter().all(|(key, _)| *key.field() == field),
                "{field:?} span points at the wrong lanes",
            );

            covered.extend(spans.iter().map(|(key, _)| *key));
        }

        // Every lane, once: a drifted offset repeats one and skips
        // another.
        let lanes = track
            .sequences_spans()
            .iter()
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();

        assert_eq!(
            covered, lanes,
            "field spans must cover every lane"
        );
    }

    mod compile {
        use typarena::id::IdGenerator;

        use crate::action::ActionMarker;

        use super::*;

        /// Distinct, monotonically increasing [`ActionId`]s.
        fn ids(count: usize) -> Vec<ActionId> {
            let mut id_gen = IdGenerator::<ActionMarker>::new();
            (0..count).map(|_| id_gen.new_id()).collect()
        }

        fn clip(id: ActionId, start: u64, dur: u64) -> ActionClip {
            ActionClip {
                id,
                start: cs(start),
                duration: cs(dur),
            }
        }

        fn seq(clips: &[ActionClip]) -> Sequence {
            let mut seq = Sequence::new(clips[0]);
            for &clip in &clips[1..] {
                seq.push(clip);
            }
            seq
        }

        /// The order `Track::bake_clips` yields.
        fn baked(track: &Track) -> Vec<ActionId> {
            track.bake_clips().map(|clip| clip.id).collect()
        }

        #[test]
        fn sorts_clips_across_sequences_by_start_time() {
            let id = ids(5);
            let track = TrackFragment::new()
                .upsert_sequence(
                    key("a"),
                    seq(&[
                        clip(id[0], 0, 100),
                        clip(id[3], 200, 100),
                    ]),
                )
                .upsert_sequence(
                    key("b"),
                    seq(&[clip(id[1], 0, 50), clip(id[2], 100, 100)]),
                )
                .upsert_sequence(
                    key("c"),
                    seq(&[clip(id[4], 50, 20)]),
                )
                .compile();

            assert_eq!(
                baked(&track),
                [id[0], id[1], id[4], id[2], id[3]],
            );
        }

        #[test]
        fn breaks_start_ties_by_action_id() {
            let id = ids(3);
            // Three sequences, every clip starting at the same instant.
            let track = TrackFragment::new()
                .upsert_sequence(key("c"), seq(&[clip(id[2], 0, 10)]))
                .upsert_sequence(key("a"), seq(&[clip(id[0], 0, 10)]))
                .upsert_sequence(key("b"), seq(&[clip(id[1], 0, 10)]))
                .compile();

            assert_eq!(baked(&track), [id[0], id[1], id[2]]);
        }

        #[test]
        fn yields_every_clip_exactly_once() {
            let id = ids(4);
            let track = TrackFragment::new()
                .upsert_sequence(
                    key("a"),
                    seq(&[
                        clip(id[0], 0, 100),
                        clip(id[1], 100, 100),
                        clip(id[2], 200, 100),
                    ]),
                )
                .upsert_sequence(
                    key("b"),
                    seq(&[clip(id[3], 50, 20)]),
                )
                .compile();

            let mut baked = baked(&track);
            baked.sort_unstable();
            let mut all = id.clone();
            all.sort_unstable();
            assert_eq!(baked, all);
        }

        #[test]
        fn sequence_clips_stay_in_order() {
            let id = ids(3);
            let track = TrackFragment::new()
                .upsert_sequence(
                    key("a"),
                    seq(&[
                        clip(id[0], 0, 100),
                        clip(id[1], 100, 100),
                        clip(id[2], 200, 100),
                    ]),
                )
                .compile();

            assert_eq!(baked(&track), [id[0], id[1], id[2]]);
        }

        /// Parallel branches append clips in authoring order, so one
        /// field can collect a later-starting clip first. Sampling
        /// binary-searches the clips and `end()` reads the last one, so
        /// compile must restore start-time order.
        #[test]
        fn clips_authored_out_of_time_order_are_sorted() {
            let id = ids(2);
            let track = TrackFragment::new()
                .upsert_sequence(
                    key("a"),
                    seq(&[clip(id[0], 100, 50)]),
                )
                .upsert_sequence(key("a"), seq(&[clip(id[1], 0, 50)]))
                .compile();

            let (_, span) = track.sequences_spans()[0];
            let starts = track
                .clips(span)
                .iter()
                .map(|clip| clip.start)
                .collect::<Vec<_>>();

            assert_eq!(starts, [cs(0), cs(100)]);
            assert_eq!(track.duration(), cs(150));
        }

        #[test]
        fn empty_track_has_no_bake_order() {
            let track = TrackFragment::new().compile();
            assert_eq!(track.bake_clips().count(), 0);
        }

        #[test]
        fn overlapping_aliasing_actions_drop_the_earlier() {
            let id = ids(2);
            // `""` (the source) aliases `.x`; the later clip overlaps.
            let track = TrackFragment::new()
                .upsert_sequence(key(""), seq(&[clip(id[0], 0, 100)]))
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[1], 50, 100)]),
                )
                .compile();

            assert_eq!(baked(&track), [id[1]]);
        }

        #[test]
        fn overlapping_clips_under_one_key_drop_the_earlier() {
            let id = ids(2);
            // Both clips land on the same `ActionKey` via an upsert
            // append and overlap in time; the later one wins outright.
            let track = TrackFragment::new()
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[0], 0, 100)]),
                )
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[1], 50, 100)]),
                )
                .compile();

            assert_eq!(baked(&track), [id[1]]);
        }

        fn subject_key(
            subject: u64,
            path: &'static str,
        ) -> ActionKey {
            ActionKey::new(
                UntypedSubjectId::placeholder_with_u64(subject),
                UntypedField::placeholder_with_path(path),
            )
        }

        #[test]
        fn different_subjects_never_conflict() {
            let id = ids(2);
            let track = TrackFragment::new()
                .upsert_sequence(
                    subject_key(1, ".x"),
                    seq(&[clip(id[0], 0, 100)]),
                )
                .upsert_sequence(
                    subject_key(2, ".x"),
                    seq(&[clip(id[1], 50, 100)]),
                )
                .compile();

            assert_eq!(baked(&track), [id[0], id[1]]);
        }

        #[test]
        fn non_aliasing_fields_never_conflict() {
            let id = ids(2);
            // `.x` and `.xy` share a prefix but not a path segment.
            let track = TrackFragment::new()
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[0], 0, 100)]),
                )
                .upsert_sequence(
                    key(".xy"),
                    seq(&[clip(id[1], 50, 100)]),
                )
                .compile();

            assert_eq!(baked(&track), [id[0], id[1]]);
        }

        #[test]
        fn touching_clips_do_not_conflict() {
            let id = ids(3);
            // Back to back, plus a zero-length set at the shared start.
            let track = TrackFragment::new()
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[0], 0, 100)]),
                )
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[1], 100, 100)]),
                )
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[2], 100, 0)]),
                )
                .compile();

            assert_eq!(baked(&track), [id[0], id[1], id[2]]);
        }

        #[test]
        fn zero_length_clip_inside_a_tween_drops_it() {
            let id = ids(2);
            let track = TrackFragment::new()
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[0], 0, 100)]),
                )
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[1], 50, 0)]),
                )
                .compile();

            assert_eq!(baked(&track), [id[1]]);
        }

        #[test]
        fn later_authored_clip_wins_even_when_it_starts_earlier() {
            let id = ids(3);
            // id[1] (authored second, starts first) drops id[0];
            // id[2] then drops id[1]. Every overlap is checked, even
            // against a clip that is itself dropped.
            let track = TrackFragment::new()
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[0], 50, 100)]),
                )
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[1], 0, 100)]),
                )
                .upsert_sequence(
                    key(""),
                    seq(&[clip(id[2], 90, 100)]),
                )
                .compile();

            assert_eq!(baked(&track), [id[2]]);
        }

        /// The previous all-pairs implementation, kept as the
        /// reference for which clips must be dropped.
        fn all_pairs_dropped(
            sequences: &[(ActionKey, Sequence)],
        ) -> HashSet<ActionId> {
            let mut removed = HashSet::new();
            for (key_a, seq_a) in sequences {
                for (key_b, seq_b) in sequences {
                    if key_a.subject_id() != key_b.subject_id()
                        || key_a.field().source_id()
                            != key_b.field().source_id()
                        || !paths_alias(
                            key_a.field().field_path(),
                            key_b.field().field_path(),
                        )
                    {
                        continue;
                    }
                    for ca in seq_a.clips.iter() {
                        for cb in seq_b.clips.iter() {
                            if cb.id > ca.id
                                && ca.start < cb.end()
                                && cb.start < ca.end()
                            {
                                removed.insert(ca.id);
                            }
                        }
                    }
                }
            }
            removed
        }

        #[test]
        fn grouped_sweep_matches_all_pairs_reference() {
            const PATHS: [&str; 5] = ["", ".x", ".y", ".x.a", ".xa"];
            // Fixed-seed xorshift: deterministic, no extra dependency.
            let mut state = 0x9E37_79B9_7F4A_7C15_u64;
            let mut next = |bound: u64| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state % bound
            };

            for _ in 0..500 {
                let count = 1 + next(24) as usize;
                let id = ids(count);
                let mut fragment = TrackFragment::new();
                for &clip_id in &id {
                    let key = subject_key(
                        next(3),
                        PATHS[next(PATHS.len() as u64) as usize],
                    );
                    // Durations include zero; starts collide often.
                    let clip = clip(clip_id, next(20), next(8));
                    fragment = fragment
                        .upsert_sequence(key, Sequence::new(clip));
                }

                let mut sequences = fragment
                    .sequences
                    .into_iter()
                    .collect::<Vec<_>>();
                let expected = all_pairs_dropped(&sequences);
                resolve_conflicts(&mut sequences);

                let kept = sequences
                    .iter()
                    .flat_map(|(_, sequence)| sequence.clips.iter())
                    .map(|clip| clip.id)
                    .collect::<HashSet<_>>();
                for clip_id in &id {
                    assert_eq!(
                        kept.contains(clip_id),
                        !expected.contains(clip_id),
                        "clip {clip_id:?} kept/dropped differently \
                         from the all-pairs reference",
                    );
                }
            }
        }

        #[cfg(feature = "diagnostics")]
        #[test]
        fn conflicts_report_the_dropped_and_winning_clips() {
            use crate::time::{Range, cs};

            let id = ids(2);
            let track = TrackFragment::new()
                .upsert_sequence(key(""), seq(&[clip(id[0], 0, 100)]))
                .upsert_sequence(
                    key(".x"),
                    seq(&[clip(id[1], 50, 100)]),
                )
                .compile();

            let conflicts = track.conflicts();
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].dropped, id[0]);
            assert_eq!(conflicts[0].winner, id[1]);
            assert_eq!(
                conflicts[0].dropped_span,
                Range {
                    start: cs(0),
                    end: cs(100)
                },
            );
            assert_eq!(
                conflicts[0].overlap,
                Range {
                    start: cs(50),
                    end: cs(100)
                },
            );
        }
    }
}
