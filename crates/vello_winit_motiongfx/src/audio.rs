//! Drives a scene's sound from the preview clock: sound plays during
//! forward playback only, and stops on a pause, seek or reverse.

use crate::{PlaybackMode, PlaybackState};
use motiongfx_audio::{AudioOutput, Score};
use std::time::Duration;

/// The scene's audio output and what the clock did on the last tick.
pub(crate) struct AudioSync {
    output: AudioOutput,
    mode: PlaybackMode,
    last: Option<PlaybackState>,
}

impl AudioSync {
    /// Open the default output device for `animation`'s sound, or `None`
    /// if it has none. A device that fails to open is reported on stderr
    /// and the preview stays silent: it is still useful without sound.
    pub(crate) fn open(
        animation: &motiongfx_2d::CompiledScene,
        mode: PlaybackMode,
    ) -> Option<Self> {
        let score = Score::new(animation);
        if score.is_empty() {
            return None;
        }
        match AudioOutput::open(score) {
            Ok(output) => Some(Self {
                output,
                mode,
                last: None,
            }),
            Err(error) => {
                eprintln!("playing without sound: {error}");
                None
            }
        }
    }

    /// The clock moved from `before` to `after` in one tick. `before` is
    /// read after any input (pause, seek) since the previous tick.
    pub(crate) fn update(
        &mut self,
        before: PlaybackState,
        after: PlaybackState,
    ) {
        for action in actions(before, after, self.last, self.mode)
            .into_iter()
            .flatten()
        {
            match action {
                Action::Advance(from, to) => {
                    self.output.advance(from, to)
                }
                Action::Stop => self.output.stop_all(),
            }
        }
        self.last = Some(after);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Advance(Duration, Duration),
    Stop,
}

/// What to tell the player for one tick from `before` to `after`, given
/// the state after the previous tick (`last`).
fn actions(
    before: PlaybackState,
    after: PlaybackState,
    last: Option<PlaybackState>,
    mode: PlaybackMode,
) -> [Option<Action>; 3] {
    let forwards =
        |state: PlaybackState| state.playing && !state.reversing;
    if !forwards(before) {
        // Stop when something changed since the last tick (a pause, a
        // seek, the turn of a ping-pong). Reaching the end of a single
        // play changes nothing, so its last notes ring out.
        let changed = last != Some(before);
        return [changed.then_some(Action::Stop), None, None];
    }
    // Resuming, or the first tick: start afresh at the current time, not
    // where the sound last stopped.
    let resumed =
        (!last.is_some_and(forwards)).then_some(Action::Stop);
    let (from, to, end) = (before.time, after.time, after.duration);
    if after.reversing {
        // Ping-pong turned at the end; nothing plays on the way back.
        [
            resumed,
            Some(Action::Advance(from, end)),
            Some(Action::Stop),
        ]
    } else if to >= from {
        [resumed, Some(Action::Advance(from, to)), None]
    } else {
        // A loop wrapped: play to the end, then on from the loop start.
        let start = match mode {
            PlaybackMode::LoopFrom(start) => start,
            _ => Duration::ZERO,
        };
        [
            resumed,
            Some(Action::Advance(from, end)),
            Some(Action::Advance(start, to)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const END: Duration = Duration::from_secs(10);

    fn at(seconds: u64, playing: bool) -> PlaybackState {
        PlaybackState {
            time: Duration::from_secs(seconds),
            duration: END,
            playing,
            reversing: false,
        }
    }

    fn run(
        before: PlaybackState,
        after: PlaybackState,
        last: Option<PlaybackState>,
        mode: PlaybackMode,
    ) -> Vec<Action> {
        actions(before, after, last, mode)
            .into_iter()
            .flatten()
            .collect()
    }

    fn advance(from: u64, to: u64) -> Action {
        Action::Advance(
            Duration::from_secs(from),
            Duration::from_secs(to),
        )
    }

    #[test]
    fn continuous_playback_advances_and_a_resume_restarts_the_clock()
    {
        let once = PlaybackMode::Once;
        let playing =
            run(at(1, true), at(2, true), Some(at(1, true)), once);
        assert_eq!(playing, [advance(1, 2)]);
        // First tick, or after a pause: re-anchor before playing on.
        let resumed =
            run(at(1, true), at(2, true), Some(at(1, false)), once);
        assert_eq!(resumed, [Action::Stop, advance(1, 2)]);
        assert_eq!(
            run(at(0, true), at(1, true), None, once)[0],
            Action::Stop
        );
    }

    #[test]
    fn pausing_or_seeking_stops_but_the_natural_end_rings_out() {
        let once = PlaybackMode::Once;
        // Paused (or seeked, which pauses) since the last tick.
        assert_eq!(
            run(at(3, false), at(3, false), Some(at(3, true)), once),
            [Action::Stop]
        );
        assert_eq!(
            run(at(7, false), at(7, false), Some(at(3, false)), once),
            [Action::Stop]
        );
        // Stopped at the end on its own: nothing changed since.
        let end = at(10, false);
        assert_eq!(run(end, end, Some(end), once), []);
    }

    #[test]
    fn a_loop_wrap_plays_to_the_end_then_from_the_loop_start() {
        let mode = PlaybackMode::LoopFrom(Duration::from_secs(4));
        assert_eq!(
            run(at(9, true), at(5, true), Some(at(9, true)), mode),
            [advance(9, 10), advance(4, 5)]
        );
    }

    #[test]
    fn ping_pong_plays_forwards_only() {
        let mode = PlaybackMode::PingPong;
        let turned = PlaybackState {
            reversing: true,
            ..at(9, true)
        };
        assert_eq!(
            run(at(9, true), turned, Some(at(9, true)), mode),
            [advance(9, 10), Action::Stop]
        );
        let back = PlaybackState {
            time: Duration::from_secs(8),
            ..turned
        };
        assert_eq!(run(turned, back, Some(turned), mode), []);
    }
}
