//! The clock that drives the editor: where in the animation it is,
//! whether and which way time moves and how fast, and the sound that
//! follows it.
//!
//! Time moves on vsync-paced frame callbacks while playing, and not at
//! all while paused. The preview and the timeline read its state; the
//! controls, keys and track change it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use vello_winit_motiongfx::{
    Playback, PlaybackConfig, PlaybackState,
};
use zgui::prelude::*;
use zgui::view::FrameHandle;
use zgui::vocab::Timestamp;

use crate::timeline::FRAME;

/// The shuttle's multipliers: each further press of J or L in the
/// direction already playing takes the next.
const SHUTTLE: [f64; 4] = [1.0, 2.0, 4.0, 8.0];

/// The clock, its sound, and what the view and preview read from it.
/// Cloned into every listener; all clones share one clock.
#[derive(Clone)]
pub(crate) struct Clock {
    inner: Rc<RefCell<ClockInner>>,
    /// What the timeline shows.
    pub(crate) state: RwSignal<PlaybackState, LocalStorage>,
    /// Play time per second: the chosen speed times the shuttle's.
    pub(crate) speed: RwSignal<f64, LocalStorage>,
    /// What the preview draws.
    wanted: Rc<Cell<Duration>>,
    timers: Timers,
    /// The pending frame callback while playing.
    frame: Rc<RefCell<Option<FrameHandle>>>,
}

struct ClockInner {
    playback: Playback,
    /// When the last playing frame ran, so the next one advances by the
    /// time between them. `None` until playback (re)starts.
    last_frame: Option<Timestamp>,
    /// Playing backwards. The shared clock only plays forwards, so it is
    /// held paused and moved by seeks while this is set.
    backward: bool,
    /// The speed chosen from the menu.
    chosen_speed: f64,
    /// What repeated J or L presses multiply it by; one of [`SHUTTLE`].
    shuttle: f64,
    #[cfg(feature = "audio")]
    audio: Option<vello_winit_motiongfx::audio::AudioSync>,
}

impl ClockInner {
    fn state(&self) -> PlaybackState {
        let mut state = self.playback.state();
        if self.backward {
            state.playing = true;
            state.reversing = true;
        }
        state
    }

    fn speed(&self) -> f64 {
        self.chosen_speed * self.shuttle
    }

    /// Tell the sound the speed, after anything that may have changed it.
    fn sync_speed(&mut self) {
        #[cfg(feature = "audio")]
        {
            let speed = self.speed();
            if let Some(audio) = &mut self.audio {
                audio.set_speed(speed);
            }
        }
    }
}

impl Clock {
    /// A silent clock for `config`, at the start and playing.
    ///
    /// # Panics
    ///
    /// Outside a window, which is what paces its frames.
    pub(crate) fn new(config: PlaybackConfig) -> Self {
        let playback = Playback::new(config);
        let start = playback.state();
        let clock = Self {
            inner: Rc::new(RefCell::new(ClockInner {
                playback,
                last_frame: None,
                backward: false,
                chosen_speed: 1.0,
                shuttle: 1.0,
                #[cfg(feature = "audio")]
                audio: None,
            })),
            state: RwSignal::new_local(start),
            speed: RwSignal::new_local(1.0),
            wanted: Rc::new(Cell::new(start.time)),
            timers: Timers::current()
                .expect("the editor is built in a window"),
            frame: Rc::new(RefCell::new(None)),
        };
        clock.publish(start);
        clock
    }

    /// Drive `audio` from this clock from now on.
    #[cfg(feature = "audio")]
    pub(crate) fn attach_audio(
        &self,
        audio: Option<vello_winit_motiongfx::audio::AudioSync>,
    ) {
        self.inner.borrow_mut().audio = audio;
    }

    /// The time the preview should draw, shared with it.
    pub(crate) fn wanted(&self) -> Rc<Cell<Duration>> {
        Rc::clone(&self.wanted)
    }

    /// Apply a control's change, then publish the new state.
    fn change(&self, edit: impl FnOnce(&mut ClockInner)) {
        let after = {
            let mut inner = self.inner.borrow_mut();
            edit(&mut inner);
            inner.sync_speed();
            inner.last_frame = None;
            let after = inner.state();
            // A pause or seek reaches the sound as a tick that didn't move,
            // which is how the winit preview reports it too.
            #[cfg(feature = "audio")]
            if let Some(audio) = &mut inner.audio {
                audio.update(after, after);
            }
            after
        };
        self.publish(after);
        self.publish_speed();
    }

    /// Change the speed without disturbing playback: the frame loop runs
    /// on and the sound keeps its place.
    fn retime(&self, edit: impl FnOnce(&mut ClockInner)) {
        {
            let mut inner = self.inner.borrow_mut();
            edit(&mut inner);
            inner.sync_speed();
        }
        self.publish_speed();
    }

    fn publish_speed(&self) {
        let speed = self.inner.borrow().speed();
        if self.speed.get_untracked() != speed {
            self.speed.set(speed);
        }
    }

    /// One playing frame at `at`.
    fn tick(&self, at: Timestamp) {
        let after = {
            let mut inner = self.inner.borrow_mut();
            let before = inner.state();
            let delta = inner
                .last_frame
                .map_or(Duration::ZERO, |last| {
                    at.saturating_since(last)
                })
                .mul_f64(inner.speed());
            inner.last_frame = Some(at);
            if inner.backward {
                let time = before.time.saturating_sub(delta);
                inner.playback.seek(time);
                inner.backward = !time.is_zero();
            } else {
                inner.playback.advance(delta);
            }
            let after = inner.state();
            #[cfg(feature = "audio")]
            if let Some(audio) = &mut inner.audio {
                audio.update(before, after);
            }
            #[cfg(not(feature = "audio"))]
            let _ = before;
            after
        };
        self.publish(after);
    }

    /// Show `state`, and keep frames coming only while time moves.
    fn publish(&self, state: PlaybackState) {
        self.wanted.set(state.time);
        self.state.set(state);
        if state.playing {
            let clock = self.clone();
            // Replacing the handle of a callback that already ran cancels
            // nothing, which is how zgui expects a frame loop to re-arm.
            let next =
                self.timers.request_frame(move |at| clock.tick(at));
            *self.frame.borrow_mut() = Some(next);
        } else {
            self.frame.borrow_mut().take();
        }
    }

    /// Pause at `time`.
    pub(crate) fn seek(&self, time: Duration) {
        self.change(|inner| {
            inner.backward = false;
            inner.shuttle = 1.0;
            inner.playback.seek(time);
        });
    }

    /// Pause `frames` frames from the current time.
    pub(crate) fn step(&self, frames: i32) {
        self.nudge(FRAME * frames.unsigned_abs(), frames >= 0);
    }

    /// Pause `by` later (or earlier).
    pub(crate) fn nudge(&self, by: Duration, later: bool) {
        let now = self.state.get_untracked().time;
        self.seek(if later {
            now.saturating_add(by)
        } else {
            now.saturating_sub(by)
        });
    }

    /// Play if paused, pause if playing either way.
    pub(crate) fn toggle(&self) {
        self.change(|inner| {
            inner.shuttle = 1.0;
            if inner.backward {
                inner.backward = false;
            } else {
                inner.playback.toggle();
            }
        });
    }

    /// Play forwards, or backwards with `forward` false; backwards stops
    /// at the start. Already playing that way, the shuttle speeds up
    /// instead, without disturbing playback.
    pub(crate) fn play(&self, forward: bool) {
        let state = self.state.get_untracked();
        if state.playing && state.reversing != forward {
            self.retime(|inner| {
                inner.shuttle = SHUTTLE
                    .into_iter()
                    .find(|&step| step > inner.shuttle)
                    .unwrap_or(inner.shuttle);
            });
            return;
        }
        self.change(|inner| {
            inner.shuttle = 1.0;
            if forward {
                inner.backward = false;
                if !inner.playback.state().playing {
                    inner.playback.toggle();
                }
            } else {
                inner.playback.pause();
                inner.backward =
                    !inner.playback.state().time.is_zero();
            }
        });
    }

    /// Play at `speed` times real time, with the shuttle back at 1x.
    pub(crate) fn set_speed(&self, speed: f64) {
        self.retime(|inner| {
            inner.chosen_speed = speed;
            inner.shuttle = 1.0;
        });
    }

    /// Scale the sound by `gain`, from 0 to 1.
    #[cfg_attr(not(feature = "audio"), allow(unused_variables))]
    pub(crate) fn set_volume(&self, gain: f32) {
        #[cfg(feature = "audio")]
        if let Some(audio) = &mut self.inner.borrow_mut().audio {
            audio.set_volume(gain);
        }
    }
}
