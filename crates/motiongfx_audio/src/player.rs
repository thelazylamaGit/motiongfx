use crate::Score;
use crate::engine::{Engine, Voice};
use crate::score::{ScoreEvent, frames};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

/// How far ahead of the mixer's position a resumed playback starts:
/// enough for one host frame plus one audio callback, so the first
/// events aren't late. Sound runs this far behind the picture.
const LATENCY: Duration = Duration::from_millis(30);
/// Room in each queue between the threads. Only a few voices are in
/// flight at once; this covers bursts.
const QUEUE: usize = 1024;
/// How long a volume change takes to reach its new level, so it doesn't
/// click.
const VOLUME_RAMP: Duration = Duration::from_millis(10);

enum Command {
    Start(Voice),
    StopAll,
}

/// The host-thread half of live playback. Each frame the host reports how
/// the play clock moved with [`advance`](Self::advance), or stops sound
/// with [`stop_all`](Self::stop_all) on a seek, pause or change of
/// direction. The
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
    /// Play time per second of sound.
    speed: f64,
    /// The master gain the mixer ramps to, as `f32` bits.
    volume: Arc<AtomicU32>,
}

/// Where continuous playback stands: the mixer frame at which playback
/// (re)started, and the sound time since then.
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
            speed: 1.0,
            volume: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
        };
        let mixer = Mixer {
            engine: Engine::new(),
            commands: receive,
            finished: send_back,
            rendered,
            volume: player.volume.clone(),
            gain: 1.0,
            ramp: 1.0
                / frames(VOLUME_RAMP, sample_rate).max(1) as f32,
        };
        (player, mixer)
    }

    /// The play clock moved from `from` to `to`: start the events the
    /// playhead reached, each at its exact offset.
    ///
    /// Forwards, those are the events that begin in `from..to`.
    /// Backwards (`to` before `from`), the score plays mirrored: an event
    /// starts when the playhead reaches its end, so the events in
    /// `to..from` come in reverse order, though each sounds forwards for
    /// its usual length.
    ///
    /// Successive calls continue seamlessly, even when `from` isn't the
    /// previous `to`: report a loop's wrap as `advance(previous, end)`
    /// then `advance(loop_start, now)`, and a ping-pong's turn as
    /// `advance(previous, end)` then `advance(end, now)`. After
    /// [`stop_all`](Self::stop_all), the next call starts afresh a moment
    /// ahead of the audio thread.
    pub fn advance(&mut self, from: Duration, to: Duration) {
        self.collect_finished();
        if self.stop_pending {
            self.stop_all();
        }
        let anchor = *self.anchor.get_or_insert_with(|| Anchor {
            frame: self.rendered.load(Ordering::Acquire)
                + frames(LATENCY, self.sample_rate),
            elapsed: Duration::ZERO,
        });
        let (rate, speed) = (self.sample_rate, self.speed);
        // Play time passes the same way in either direction, and turns
        // into sound time at the playing speed.
        let at = |time: Duration| {
            anchor.frame
                + frames(
                    anchor.elapsed
                        + time.abs_diff(from).div_f64(speed),
                    rate,
                )
        };
        let commands = &mut self.commands;
        let mut start = |event: &ScoreEvent, reached: Duration| {
            let start = at(reached);
            let release =
                start + frames(event.length.div_f64(speed), rate);
            if let Some(voice) =
                event.sound.voice(rate, start, release)
            {
                // A full queue means the audio thread has stalled; skip
                // the event rather than block.
                let _ = commands.push(Command::Start(voice));
            }
        };
        if to >= from {
            for event in self.score.starting_in(from, to) {
                start(event, event.start);
            }
        } else {
            for event in self.score.ending_in(to, from) {
                start(event, event.end());
            }
        }
        self.anchor = Some(Anchor {
            elapsed: anchor.elapsed
                + from.abs_diff(to).div_f64(speed),
            ..anchor
        });
    }

    /// Play at `speed` times real time from the next
    /// [`advance`](Self::advance) on. Events come sooner or later and
    /// notes are held shorter or longer, but every sound keeps its pitch:
    /// a change of tempo, not of tape speed.
    ///
    /// # Panics
    ///
    /// If `speed` isn't a positive, finite number.
    #[track_caller]
    pub fn set_speed(&mut self, speed: f64) {
        assert!(
            speed.is_finite() && speed > 0.0,
            "a playing speed must be positive and finite, not {speed}"
        );
        self.speed = speed;
    }

    /// Scale the whole mix by `gain`, from silent at 0 to as rendered at
    /// 1 (values outside are clamped). The mixer ramps to it in at most
    /// 10 ms.
    pub fn set_volume(&mut self, gain: f32) {
        let gain = if gain.is_nan() {
            0.0
        } else {
            gain.clamp(0.0, 1.0)
        };
        self.volume.store(gain.to_bits(), Ordering::Relaxed);
    }

    /// Fade out every voice over 5 ms and drop any waiting to start, for a
    /// seek, pause or change of direction. Nothing plays until the next
    /// [`advance`](Self::advance), which starts only events the playhead
    /// reaches after its `from`; notes already under way are not
    /// restarted.
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
    volume: Arc<AtomicU32>,
    /// The master gain right now, ramping towards `volume`.
    gain: f32,
    /// How far the gain moves per frame.
    ramp: f32,
}

impl Mixer {
    /// Render the next `out.len() / 2` frames into `out` as interleaved
    /// stereo, overwriting it. Samples are clamped to ±1, then scaled by
    /// the volume.
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
        let target =
            f32::from_bits(self.volume.load(Ordering::Relaxed));
        if self.gain != target || target != 1.0 {
            for [left, right] in out.as_chunks_mut::<2>().0 {
                self.gain +=
                    (target - self.gain).clamp(-self.ramp, self.ramp);
                *left *= self.gain;
                *right *= self.gain;
            }
        }
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
