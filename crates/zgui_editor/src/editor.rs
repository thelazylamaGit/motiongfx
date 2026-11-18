//! The editor's view: the preview above a timeline, and the clock that
//! drives both.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use vello::kurbo::Size;
use vello_winit_motiongfx::{
    Playback, PlaybackConfig, PlaybackMode, PlaybackState,
};
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui::view::FrameHandle;
use zgui::vocab::Timestamp;
use zgui_ui_tokens::prelude::*;

use crate::preview::Preview;
use crate::timeline::{FRAME, Viewport, ZOOM_STEP, timeline};

/// How far Shift with an arrow key moves the time.
const SHIFT_STEP: Duration = Duration::from_secs(1);

/// What [`crate::Window::run`] hands to the view.
pub(crate) struct Setup {
    pub(crate) animation: motiongfx_2d::CompiledScene,
    pub(crate) renderer: vello_motiongfx::Renderer,
    pub(crate) canvas: Size,
    pub(crate) playback_mode: PlaybackMode,
    #[cfg(feature = "audio")]
    pub(crate) audio: bool,
}

/// The clock, its sound, and what the view and preview read from it.
/// Cloned into every listener; all clones share one clock.
#[derive(Clone)]
pub(crate) struct Clock {
    inner: Rc<RefCell<ClockInner>>,
    /// What the timeline shows.
    pub(crate) state: RwSignal<PlaybackState, LocalStorage>,
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
}

impl Clock {
    /// Apply a control's change, then publish the new state.
    fn change(&self, edit: impl FnOnce(&mut ClockInner)) {
        let after = {
            let mut inner = self.inner.borrow_mut();
            edit(&mut inner);
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
    }

    /// One playing frame at `at`.
    fn tick(&self, at: Timestamp) {
        let after = {
            let mut inner = self.inner.borrow_mut();
            let before = inner.state();
            let delta =
                inner.last_frame.map_or(Duration::ZERO, |last| {
                    at.saturating_since(last)
                });
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
            inner.playback.seek(time);
        });
    }

    /// Pause `frames` frames from the current time.
    pub(crate) fn step(&self, frames: i32) {
        self.nudge(FRAME * frames.unsigned_abs(), frames >= 0);
    }

    /// Pause `by` later (or earlier).
    fn nudge(&self, by: Duration, later: bool) {
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
            if inner.backward {
                inner.backward = false;
            } else {
                inner.playback.toggle();
            }
        });
    }

    /// Play forwards, or backwards with `forward` false; backwards stops
    /// at the start. Already playing that way, nothing changes, so the
    /// frame loop runs on without a hitch.
    pub(crate) fn play(&self, forward: bool) {
        let state = self.state.get_untracked();
        if state.playing && state.reversing != forward {
            return;
        }
        self.change(|inner| {
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
}

/// The editor: the preview filling the window above the timeline.
pub(crate) fn editor(setup: Setup) -> impl IntoView {
    let viewport = RwSignal::new_local(Viewport::whole(
        setup.animation.duration(),
    ));
    let config = PlaybackConfig::new(setup.animation.duration())
        .with_mode(setup.playback_mode);
    let playback = Playback::new(config);
    let start = playback.state();
    #[cfg(feature = "audio")]
    let audio = setup
        .audio
        .then(|| {
            vello_winit_motiongfx::audio::AudioSync::open(
                &setup.animation,
                setup.playback_mode,
            )
        })
        .flatten();

    let wanted = Rc::new(Cell::new(start.time));
    let clock = Clock {
        inner: Rc::new(RefCell::new(ClockInner {
            playback,
            last_frame: None,
            backward: false,
            #[cfg(feature = "audio")]
            audio,
        })),
        state: RwSignal::new_local(start),
        wanted: Rc::clone(&wanted),
        timers: Timers::current()
            .expect("the editor is built in a window"),
        frame: Rc::new(RefCell::new(None)),
    };
    clock.publish(start);

    let preview = Preview::new(
        setup.animation,
        setup.renderer,
        setup.canvas,
        wanted,
    );

    // Shortcuts work before anything is focused: register the root for
    // keys nothing else takes.
    let root = NodeRef::new();
    let registration = RenderEffect::new(
        move |previous: Option<Option<WindowShortcut>>| {
            drop(previous);
            root.get();
            root.window_shortcut()
        },
    );
    on_cleanup_local(move || drop(registration));

    let keys = clock.clone();
    // K toggles playback. While it is held, J and L step one frame, as in
    // most editors; the time K was pressed at is kept so a step lands a
    // frame from there, not from wherever the toggle has since played to.
    let holding_k: Rc<Cell<Option<Duration>>> =
        Rc::new(Cell::new(None));
    let released_k = Rc::clone(&holding_k);
    let zoom = move |change: fn(Viewport, f64) -> Viewport,
                     time: Duration| {
        viewport
            .update(|view| *view = change(*view, time.as_secs_f64()));
    };
    view! {
        ThemeProvider(
            scheme = RwSignal::new_local(ColorScheme::Dark),
            light = Signal::derive_local(|| Preset::default().light()),
            dark = Signal::derive_local(|| Preset::default().dark())
        ) {
            column(
                class = "editor",
                node_ref = root,
                on:key_down = move |ev| {
                    let time = keys.state.get_untracked().time;
                    if !ev.modifiers.primary() {
                        match ev.physical.code() {
                            Some(KeyCode::KeyJ | KeyCode::KeyL) => {
                                let forward = ev.physical.code() == Some(KeyCode::KeyL);
                                if let Some(pressed_at) = holding_k.get() {
                                    if keys.state.get_untracked().playing {
                                        keys.seek(pressed_at);
                                    }
                                    keys.step(if forward { 1 } else { -1 });
                                } else if !ev.repeat {
                                    keys.play(forward);
                                }
                                return;
                            }
                            Some(KeyCode::KeyK) => {
                                if !ev.repeat {
                                    holding_k.set(Some(time));
                                    keys.toggle();
                                }
                                return;
                            }
                            Some(KeyCode::Equal | KeyCode::NumpadAdd) => {
                                return zoom(|view, at| view.zoomed_around(ZOOM_STEP, at), time);
                            }
                            Some(KeyCode::Minus | KeyCode::NumpadSubtract) => {
                                return zoom(|view, at| view.zoomed_around(1.0 / ZOOM_STEP, at), time);
                            }
                            Some(KeyCode::Backslash) => {
                                return zoom(|view, _| view.fitted(), time);
                            }
                            _ => {}
                        }
                    }
                    let later = match &ev.key {
                        Key::Named(NamedKey::Space) => return keys.toggle(),
                        Key::Named(NamedKey::Home) => return keys.seek(Duration::ZERO),
                        Key::Named(NamedKey::End) => {
                            return keys.seek(keys.state.get_untracked().duration);
                        }
                        Key::Named(NamedKey::ArrowLeft) => false,
                        Key::Named(NamedKey::ArrowRight) => true,
                        _ => return,
                    };
                    if ev.modifiers.shift() {
                        keys.nudge(SHIFT_STEP, later);
                    } else {
                        keys.step(if later { 1 } else { -1 });
                    }
                },
                on:key_up = move |ev| {
                    if ev.physical.code() == Some(KeyCode::KeyK) {
                        released_k.set(None);
                    }
                }
            ) {
                {zgui::elements::surface().class("preview").renderer(preview).into_view()}
                {timeline(clock, viewport)}
            }
        }
    }
}

/// How the editor looks.
pub(crate) const SHEET: &str = zgui::css!(
    ":root {
        background-color: #111318;
        color: #e8ecf4;
        font-family: sans-serif;
    }

    .editor {
        width: 100%;
        height: 100%;
    }

    .preview {
        flex-grow: 1;
        min-height: 0;
    }"
);
