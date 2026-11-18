//! Drives a scene's sound from the preview clock: sound plays while time
//! moves, mirrored while it moves backwards, and stops on a pause, seek
//! or change of direction.

use crate::{PlaybackMode, PlaybackState};
use motiongfx_audio::{AudioOutput, OutputError, Score};
use std::time::Duration;

/// The scene's audio output and what the clock did on the last tick.
pub struct AudioSync {
    output: AudioOutput,
    mode: PlaybackMode,
    last: Option<PlaybackState>,
}

impl AudioSync {
    /// Open the default output device for `animation`'s sound, or `None`
    /// if it has none.
    ///
    /// # Errors
    ///
    /// If the scene has sound but the device fails to open. A preview is
    /// still useful without sound, so hosts report this and play on.
    pub fn open(
        animation: &motiongfx_2d::CompiledScene,
        mode: PlaybackMode,
    ) -> Result<Option<Self>, OutputError> {
        let score = Score::new(animation);
        if score.is_empty() {
            return Ok(None);
        }
        let output = AudioOutput::open(score)?;
        Ok(Some(Self {
            output,
            mode,
            last: None,
        }))
    }

    /// Play at `speed` times real time; see
    /// [`Player::set_speed`](motiongfx_audio::Player::set_speed).
    ///
    /// # Panics
    ///
    /// If `speed` isn't a positive, finite number.
    #[track_caller]
    pub fn set_speed(&mut self, speed: f64) {
        self.output.set_speed(speed);
    }

    /// Scale the whole mix by `gain`, from 0 to 1; see
    /// [`Player::set_volume`](motiongfx_audio::Player::set_volume).
    pub fn set_volume(&mut self, gain: f32) {
        self.output.set_volume(gain);
    }

    /// The clock moved from `before` to `after` in one tick. `before` is
    /// read after any input (pause, seek) since the previous tick.
    pub fn update(
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
    if !before.playing {
        // Stop when something changed since the last tick (a pause, a
        // seek). Reaching the end of a single play changes nothing, so
        // its last notes ring out.
        let changed = last != Some(before);
        return [changed.then_some(Action::Stop), None, None];
    }
    // Resuming, the first tick, or a change of direction from the
    // controls: start afresh at the current time, not where the sound
    // last stopped. A ping-pong's own turn happens within a tick, below,
    // and plays on.
    let continuing = last.is_some_and(|last| {
        last.playing && last.reversing == before.reversing
    });
    let resumed = (!continuing).then_some(Action::Stop);
    let (from, to, end) = (before.time, after.time, after.duration);
    let turned = after.playing && after.reversing != before.reversing;
    if turned {
        // A ping-pong turned at one end and is on its way back.
        let edge = if before.reversing {
            Duration::ZERO
        } else {
            end
        };
        [
            resumed,
            Some(Action::Advance(from, edge)),
            Some(Action::Advance(edge, to)),
        ]
    } else if before.reversing || to >= from {
        // Backwards, or forwards without wrapping; a backward play that
        // reached the start has stopped there, with nothing to wrap.
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
    fn ping_pong_plays_on_through_both_turns() {
        let mode = PlaybackMode::PingPong;
        let back = |seconds| PlaybackState {
            reversing: true,
            ..at(seconds, true)
        };
        // Turning at the end plays up to it, then back from it.
        assert_eq!(
            run(at(9, true), back(9), Some(at(9, true)), mode),
            [advance(9, 10), advance(10, 9)]
        );
        assert_eq!(
            run(back(9), back(8), Some(back(9)), mode),
            [advance(9, 8)]
        );
        // And at the start, back down to it, then forwards again.
        assert_eq!(
            run(back(1), at(1, true), Some(back(1)), mode),
            [advance(1, 0), advance(0, 1)]
        );
    }

    #[test]
    fn reversing_from_the_controls_restarts_the_sound() {
        let once = PlaybackMode::Once;
        let back = PlaybackState {
            reversing: true,
            ..at(5, true)
        };
        let earlier = PlaybackState {
            time: Duration::from_secs(4),
            ..back
        };
        assert_eq!(
            run(back, earlier, Some(at(5, true)), once),
            [Action::Stop, advance(5, 4)]
        );
        // Reaching the start stops there without wrapping.
        assert_eq!(
            run(earlier, at(0, false), Some(earlier), once),
            [advance(4, 0)]
        );
    }
}
