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
use crate::timeline::{FRAME, timeline};

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
    #[cfg(feature = "audio")]
    audio: Option<vello_winit_motiongfx::audio::AudioSync>,
}

impl Clock {
    /// Apply a control's change, then publish the new state.
    fn change(&self, edit: impl FnOnce(&mut Playback)) {
        let after = {
            let mut inner = self.inner.borrow_mut();
            edit(&mut inner.playback);
            inner.last_frame = None;
            let after = inner.playback.state();
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
            let before = inner.playback.state();
            let delta =
                inner.last_frame.map_or(Duration::ZERO, |last| {
                    at.saturating_since(last)
                });
            inner.last_frame = Some(at);
            inner.playback.advance(delta);
            let after = inner.playback.state();
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
        self.change(|playback| playback.seek(time));
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

    pub(crate) fn toggle(&self) {
        self.change(Playback::toggle);
    }
}

/// The editor: the preview filling the window above the timeline.
pub(crate) fn editor(setup: Setup) -> impl IntoView {
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
                }
            ) {
                {zgui::elements::surface().class("preview").renderer(preview).into_view()}
                {timeline(clock)}
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
