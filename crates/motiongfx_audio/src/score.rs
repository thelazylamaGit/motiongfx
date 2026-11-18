use crate::engine::{BLOCK, Engine};
use crate::sound::Sound;
use motiongfx_authoring::CompiledScene;
use std::time::Duration;

/// The sound events of one compiled scene, sorted by start time. Play it
/// live with a [`Player`](crate::Player), or [`render`](Self::render) it
/// offline.
///
/// # Examples
///
/// ```
/// use motiongfx::prelude::ms;
/// use motiongfx_audio::{Instrument, Score};
/// use motiongfx_audio::fundsp::prelude32::{adsr_live, sine_hz};
/// use motiongfx_authoring::{Scene, chain, wait};
///
/// let sine = Instrument::synth(|hz| sine_hz(hz) * adsr_live(0.01, 0.1, 0.6, 0.2));
/// let scene = Scene::<()>::new().compile([chain([wait(ms(100)), sine.note(60)])]);
/// let audio = Score::new(&scene).render(48_000);
/// // Silent until the note starts at 100 ms (4800 stereo frames in).
/// assert!(audio[..2 * 4800].iter().all(|&s| s == 0.0));
/// assert!(audio[2 * 4800..].iter().any(|&s| s != 0.0));
/// ```
#[derive(Clone)]
pub struct Score {
    events: Box<[ScoreEvent]>,
    /// Indices into `events`, sorted by when each ends, for playing
    /// backwards.
    by_end: Box<[u32]>,
    duration: Duration,
}

#[derive(Clone)]
pub(crate) struct ScoreEvent {
    pub(crate) start: Duration,
    pub(crate) length: Duration,
    pub(crate) sound: Sound,
}

impl ScoreEvent {
    pub(crate) fn end(&self) -> Duration {
        self.start + self.length
    }
}

impl Score {
    /// The sound events of `scene`: every note and sample play compiled
    /// into it.
    pub fn new<M: 'static>(scene: &CompiledScene<M>) -> Self {
        let events = scene
            .events::<Sound>()
            .map(|event| ScoreEvent {
                start: event.start,
                length: event.length,
                sound: event.payload.clone(),
            })
            .collect::<Box<[ScoreEvent]>>();
        let mut by_end: Box<[u32]> = (0..events.len())
            .map(|index| {
                u32::try_from(index).expect("fewer than 2^32 events")
            })
            .collect();
        by_end.sort_by_key(|&index| events[index as usize].end());
        Self {
            events,
            by_end,
            duration: scene.duration(),
        }
    }

    /// Whether the scene has no sound events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// How long the scene runs.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// The events that start in `from..to`.
    pub(crate) fn starting_in(
        &self,
        from: Duration,
        to: Duration,
    ) -> &[ScoreEvent] {
        let first = self.events.partition_point(|e| e.start < from);
        let end = self.events.partition_point(|e| e.start < to);
        &self.events[first..end.max(first)]
    }

    /// The events that end in `after..=until`, exclusive of `after`: what
    /// the playhead reaches moving backwards from `until` to `after`.
    pub(crate) fn ending_in(
        &self,
        after: Duration,
        until: Duration,
    ) -> impl Iterator<Item = &ScoreEvent> {
        let end = |index: &u32| self.events[*index as usize].end();
        let first = self.by_end.partition_point(|i| end(i) <= after);
        let last = self.by_end.partition_point(|i| end(i) <= until);
        self.by_end[first..last.max(first)]
            .iter()
            .map(|&index| &self.events[index as usize])
    }

    /// Render every event offline into interleaved stereo at
    /// `sample_rate`: from time zero until the scene has ended and every
    /// voice has fallen silent (at most 10 s later), rounded up to a
    /// whole block of 64 frames. Uses the same voices and mixing as live
    /// playback, with each event at its exact frame.
    ///
    /// # Panics
    ///
    /// If `sample_rate` is zero.
    pub fn render(&self, sample_rate: u32) -> Vec<f32> {
        assert!(sample_rate > 0, "a sample rate must be positive");
        let to_frame = |time: Duration| frames(time, sample_rate);
        let end = to_frame(self.duration);
        let limit = end + u64::from(sample_rate) * 10;
        let mut engine = Engine::new();
        let mut events = self.events.iter().peekable();
        let mut out = Vec::new();
        let mut discard = drop;
        loop {
            let block_end = engine.frame() + BLOCK as u64;
            while let Some(event) = events
                .next_if(|event| to_frame(event.start) < block_end)
            {
                let start = to_frame(event.start);
                let release = start + to_frame(event.length);
                if let Some(voice) =
                    event.sound.voice(sample_rate, start, release)
                {
                    // Full only if hundreds start in one block; most of
                    // them would be stolen at once anyway.
                    let _ = engine.schedule(voice);
                }
            }
            let finished = engine.frame() >= end
                && events.peek().is_none()
                && engine.is_idle();
            if finished || engine.frame() >= limit {
                return out;
            }
            let written = out.len();
            out.resize(written + BLOCK * 2, 0.0);
            engine.render(&mut out[written..], &mut discard);
        }
    }
}

impl std::fmt::Debug for Score {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Score")
            .field("events", &self.events.len())
            .field("duration", &self.duration)
            .finish()
    }
}

/// `time` in whole frames at `rate`, rounded to the nearest.
pub(crate) fn frames(time: Duration, rate: u32) -> u64 {
    // Exact integer maths: a float would drift over long scenes.
    let nanos = time.as_nanos() * u128::from(rate);
    u64::try_from((nanos + 500_000_000) / 1_000_000_000)
        .expect("a scene shorter than millions of years")
}
