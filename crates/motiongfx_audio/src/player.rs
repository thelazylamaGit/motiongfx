use crate::Score;
use crate::engine::{Engine, Voice};
use crate::score::frames;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// How far ahead of the mixer's position a resumed playback starts:
/// enough for one host frame plus one audio callback, so the first
/// events aren't late. Sound runs this far behind the picture.
const LATENCY: Duration = Duration::from_millis(30);
/// Room in each queue between the threads. Only a few voices are in
/// flight at once; this covers bursts.
const QUEUE: usize = 1024;

enum Command {
    Start(Voice),
    StopAll,
}

/// The host-thread half of live playback. Each frame the host reports how
/// the play clock moved with [`advance`](Self::advance), or stops sound
/// with [`stop_all`](Self::stop_all) on a seek, pause or reverse. The
/// [`Mixer`] it was created with renders the sound on the audio thread.
///
/// Voices are built here, so the audio thread never allocates, and
/// finished voices come back here to be freed.
pub struct Player {
    score: Score,
    sample_rate: u32,
    commands: rtrb::Producer<Command>,
    finished: rtrb::Consumer<Voice>,
    /// Frames the mixer has rendered.
    rendered: Arc<AtomicU64>,
    anchor: Option<Anchor>,
    /// A `stop_all` that didn't fit in the queue, retried next time.
    stop_pending: bool,
}

/// Where continuous playback stands: the mixer frame at which playback
/// (re)started, and the play time since then.
#[derive(Clone, Copy)]
struct Anchor {
    frame: u64,
    elapsed: Duration,
}

impl Player {
    /// A player for `score` and the mixer that renders its sound at
    /// `sample_rate`. Move the mixer to the audio thread.
    ///
    /// # Panics
    ///
    /// If `sample_rate` is zero.
    pub fn new(score: Score, sample_rate: u32) -> (Player, Mixer) {
        assert!(sample_rate > 0, "a sample rate must be positive");
        let (commands, receive) = rtrb::RingBuffer::new(QUEUE);
        let (send_back, finished) = rtrb::RingBuffer::new(QUEUE);
        let rendered = Arc::new(AtomicU64::new(0));
        let player = Player {
            score,
            sample_rate,
            commands,
            finished,
            rendered: rendered.clone(),
            anchor: None,
            stop_pending: false,
        };
        let mixer = Mixer {
            engine: Engine::new(),
            commands: receive,
            finished: send_back,
            rendered,
        };
        (player, mixer)
    }

    /// The play clock moved forwards from `from` to `to`: start the events
    /// that begin in `from..to`, each at its exact offset.
    ///
    /// Successive calls continue seamlessly, even when `from` isn't the
    /// previous `to`: report a loop's wrap as `advance(previous, end)`
    /// then `advance(loop_start, now)`. After [`stop_all`](Self::stop_all),
    /// the next call starts afresh a moment ahead of the audio thread.
    /// A range with `to` before `from` is ignored.
    pub fn advance(&mut self, from: Duration, to: Duration) {
        self.collect_finished();
        if self.stop_pending {
            self.stop_all();
        }
        if to < from {
            return;
        }
        let anchor = *self.anchor.get_or_insert_with(|| Anchor {
            frame: self.rendered.load(Ordering::Acquire)
                + frames(LATENCY, self.sample_rate),
            elapsed: Duration::ZERO,
        });
        let at = |time: Duration| {
            anchor.frame
                + frames(
                    anchor.elapsed + (time - from),
                    self.sample_rate,
                )
        };
        for event in self.score.starting_in(from, to) {
            let start = at(event.start);
            let release =
                start + frames(event.length, self.sample_rate);
            let Some(voice) =
                event.sound.voice(self.sample_rate, start, release)
            else {
                continue;
            };
            // A full queue means the audio thread has stalled; skip the
            // event rather than block.
            let _ = self.commands.push(Command::Start(voice));
        }
        self.anchor = Some(Anchor {
            elapsed: anchor.elapsed + (to - from),
            ..anchor
        });
    }

    /// Fade out every voice over 5 ms and drop any waiting to start, for a
    /// seek, pause or reverse. Nothing plays until the next
    /// [`advance`](Self::advance), which starts only events that begin
    /// after its `from`; notes already under way are not restarted.
    pub fn stop_all(&mut self) {
        self.collect_finished();
        self.anchor = None;
        self.stop_pending =
            self.commands.push(Command::StopAll).is_err();
    }

    /// Free voices the mixer has finished with.
    fn collect_finished(&mut self) {
        while self.finished.pop().is_ok() {}
    }
}

impl std::fmt::Debug for Player {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("score", &self.score)
            .field("sample_rate", &self.sample_rate)
            .finish_non_exhaustive()
    }
}

/// The audio-thread half of live playback: owns the sounding voices and
/// mixes them. Call [`render`](Self::render) from the audio callback. It
/// never allocates, frees, locks or blocks.
pub struct Mixer {
    engine: Engine,
    commands: rtrb::Consumer<Command>,
    finished: rtrb::Producer<Voice>,
    rendered: Arc<AtomicU64>,
}

impl Mixer {
    /// Render the next `out.len() / 2` frames into `out` as interleaved
    /// stereo, overwriting it. Samples are clamped to ±1.
    pub fn render(&mut self, out: &mut [f32]) {
        let finished = &mut self.finished;
        // Freeing happens on the host thread; only if that queue is full
        // (the host stopped collecting) is a voice freed here.
        let mut discard = |voice| {
            let _ = finished.push(voice);
        };
        while let Ok(command) = self.commands.pop() {
            match command {
                Command::Start(voice) => {
                    if let Err(voice) = self.engine.schedule(voice) {
                        discard(voice);
                    }
                }
                Command::StopAll => {
                    self.engine.stop_all(&mut discard)
                }
            }
        }
        self.engine.render(out, &mut discard);
        self.rendered.store(self.engine.frame(), Ordering::Release);
    }
}

impl std::fmt::Debug for Mixer {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Mixer")
            .field("frame", &self.engine.frame())
            .finish_non_exhaustive()
    }
}
