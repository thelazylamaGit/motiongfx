//! The editor's view: the preview above the timeline, the keyboard
//! shortcuts, and the theme around both.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use vello::kurbo::Size;
use vello_winit_motiongfx::{PlaybackConfig, PlaybackMode};
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_ui_tokens::prelude::ColorScheme;

use crate::clock::Clock;
use crate::controls::{Sound, SoundStatus};
use crate::preview::Preview;
use crate::theme::EditorThemeProps;
use crate::timeline::{Viewport, ZOOM_STEP, timeline};

/// How far Shift with an arrow key moves the time.
const SHIFT_STEP: Duration = Duration::from_secs(1);

/// What [`crate::Window::run`] hands to the view.
pub(crate) struct Setup {
    pub(crate) animation: motiongfx_2d::CompiledScene,
    pub(crate) renderer: vello_motiongfx::Renderer,
    pub(crate) canvas: Size,
    pub(crate) playback_mode: PlaybackMode,
    pub(crate) color_scheme: ColorScheme,
    #[cfg(feature = "audio")]
    pub(crate) audio: bool,
}

/// The editor: the preview filling the window above the timeline.
pub(crate) fn editor(setup: Setup) -> impl IntoView {
    let viewport = RwSignal::new_local(Viewport::whole(
        setup.animation.duration(),
    ));
    let config = PlaybackConfig::new(setup.animation.duration())
        .with_mode(setup.playback_mode);
    #[cfg(feature = "audio")]
    let (audio, status) = if setup.audio {
        match vello_winit_motiongfx::audio::AudioSync::open(
            &setup.animation,
            setup.playback_mode,
        ) {
            Ok(Some(audio)) => (Some(audio), SoundStatus::Playing),
            Ok(None) => (None, SoundStatus::None),
            Err(error) => {
                eprintln!("playing without sound: {error}");
                (None, SoundStatus::NoDevice)
            }
        }
    } else {
        (None, SoundStatus::None)
    };
    #[cfg(not(feature = "audio"))]
    let status = SoundStatus::None;

    let clock = Clock::new(config);
    #[cfg(feature = "audio")]
    clock.attach_audio(audio);

    let preview = Preview::new(
        setup.animation,
        setup.renderer,
        setup.canvas,
        clock.wanted(),
    );
    let sound = Sound::new(status, &clock);
    let surface = zgui::elements::surface()
        .class("preview")
        .renderer(preview)
        .into_view();
    view! {
        EditorTheme(scheme = setup.color_scheme) {
            {workspace(clock, viewport, sound, surface)}
        }
    }
}

/// `preview` above the timeline for `clock`, with the editor's keyboard
/// shortcuts. Everything in the window but the theme, so a test can open
/// it with a stand-in for the GPU preview.
pub(crate) fn workspace(
    clock: Clock,
    viewport: RwSignal<Viewport, LocalStorage>,
    sound: Sound,
    preview: impl IntoView,
) -> impl IntoView {
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
                            Some(KeyCode::KeyM) if !ev.repeat => return sound.toggle_mute(),
                            _ => {}
                        }
                    }
                    let later = match &ev.key {
                        Key::Named(NamedKey::Space) => {
                            // Space always plays and pauses, as in most
                            // editors. Without this, zgui would also press
                            // whichever button was last clicked, since a
                            // click focuses it and Space activates what is
                            // focused. Enter still does.
                            ev.prevent_default();
                            return keys.toggle();
                        }
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
                {preview}
                {timeline(clock, viewport, sound)}
            }
    }
}
