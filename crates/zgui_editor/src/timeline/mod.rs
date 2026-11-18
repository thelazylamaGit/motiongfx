//! The timeline under the preview: a transport bar (buttons, timecode,
//! frame readout, speed and volume) above a track with a ruler, a lane and
//! a draggable playhead. The track zooms from the whole animation down to
//! a few frames.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`viewport`] | the stretch of time in view, and how zoom, pan and playback move it |
//! | [`pointer`] | scrubbing, the navigator, scrolling past the edges, the wheel |
//! | [`draw`] | what the track's canvases draw, and the track's layout constants |
//! | [`ruler`] | tick spacing and labels |
//! | [`timecode`] | frames, the timecode and the frame readout |
//! | [`style`] | the timeline's style sheet |

mod draw;
mod pointer;
mod ruler;
mod style;
mod timecode;
mod viewport;

pub(crate) use style::SHEET;
pub(crate) use timecode::FRAME;
pub(crate) use viewport::{Viewport, ZOOM_STEP};

use std::time::Duration;

use zgui::geom::CssPx;
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

use crate::clock::Clock;
use crate::controls::{Sound, speed_menu, volume_control};
use crate::icons::{
    FRAME_BACK, FRAME_FORWARD, PAUSE, PLAY, TO_END, TO_START,
};
use pointer::{Spot, TrackPointer};
use timecode::{meta_text, timecode};

/// The timeline's height in logical pixels, added below the canvas when
/// sizing the window. The style sheet's `.timeline` uses the same number.
pub(crate) const HEIGHT: f64 = 104.0;

/// CSS pixels per wheel notch, for wheels that report notches.
const LINE: f32 = 40.0;

/// The timeline for `clock`, showing `viewport`, with the volume control
/// for `sound`.
pub(crate) fn timeline(
    clock: Clock,
    viewport: RwSignal<Viewport, LocalStorage>,
    sound: Sound,
) -> impl IntoView {
    let state = clock.state;
    let duration = state.get_untracked().duration;
    let pointer = TrackPointer::new(clock.clone(), viewport);

    // Turn the page when the playhead leaves the view, as playback runs.
    // Only the time is tracked, so panning away while paused stays put,
    // and a drag decides what is in view while it lasts.
    let following = pointer.clone();
    let follow = RenderEffect::new(move |_| {
        let time = state.get().time.as_secs_f64();
        if following.is_dragging() {
            return;
        }
        let now = viewport.get_untracked();
        let next = now.following(time);
        if next != now {
            viewport.set(next);
        }
    });
    on_cleanup_local(move || drop(follow));

    // The track's width, which decides how many ruler labels fit. The
    // marks canvas is told it with every layout, and passes it on.
    let track_width = RwSignal::new_local(0.0);
    let marks =
        zgui::elements::canvas().class("marks").draw(move |cx| {
            let view = viewport.get();
            let (width, height) = (
                f64::from(cx.size.width.0),
                f64::from(cx.size.height.0),
            );
            if width <= 0.0 {
                return;
            }
            if track_width.get_untracked() != width {
                track_width.set(width);
            }
            draw::marks(cx, view, width, height);
        });
    let playhead =
        zgui::elements::canvas().class("playhead").draw(move |cx| {
            let (width, height) = (
                f64::from(cx.size.width.0),
                f64::from(cx.size.height.0),
            );
            if width > 0.0 {
                draw::playhead(
                    cx,
                    viewport.get(),
                    state.get(),
                    width,
                    height,
                );
            }
        });

    let track = NodeRef::new();
    #[cfg(test)]
    crate::headless::TRACK.set(Some(track));
    let (down, moved, up, cancelled, wheel) = (
        pointer.clone(),
        pointer.clone(),
        pointer.clone(),
        pointer.clone(),
        pointer,
    );

    let (start, back, forward, end, play) = (
        clock.clone(),
        clock.clone(),
        clock.clone(),
        clock.clone(),
        clock.clone(),
    );
    let buttons = view! {
        row(class = "transport__buttons") {
            {transport_button(TO_START, "Go to start", "transport__jump", move || start.seek(Duration::ZERO))}
            {transport_button(FRAME_BACK, "Previous frame", "transport__step", move || back.step(-1))}
            Button(
                variant = ButtonVariant::Secondary,
                size = ButtonSize::Icon,
                class = "transport__play",
                on:click = move |_| play.toggle()
            ) {
                Icon(
                    icon = Signal::derive_local(move || {
                        if state.get().playing { PAUSE } else { PLAY }
                    }),
                    label = "Play or pause"
                )
            }
            {transport_button(FRAME_FORWARD, "Next frame", "transport__step", move || forward.step(1))}
            {transport_button(TO_END, "Go to end", "transport__jump", move || end.seek(duration))}
        }
    };

    view! {
        column(class = "timeline") {
            row(class = "transport") {
                {buttons}
                text(class = "timecode") {{move || timecode(state.get().time)}}
                text(class = "timecode timecode--total") {{format!("/ {}", timecode(duration))}}
                box(class = "transport__spacer") {}
                text(class = "transport__meta") {{move || meta_text(state.get(), viewport.get())}}
                row(class = "transport__controls") {
                    {speed_menu(clock)}
                    {volume_control(sound)}
                }
            }
            box(
                class = "track",
                node_ref = track,
                on:pointer_down = move |ev| {
                    if let Some(spot) = Spot::locate(track, ev.position.x.0, ev.position.y.0) {
                        ev.capture_pointer();
                        down.press(spot);
                    }
                },
                on:pointer_move = move |ev| {
                    if let Some(spot) = Spot::locate(track, ev.position.x.0, ev.position.y.0) {
                        moved.drag_to(spot);
                    }
                },
                on:pointer_up = move |ev| {
                    up.release();
                    ev.release_pointer();
                },
                on:pointer_cancel = move |ev| {
                    cancelled.release();
                    ev.release_pointer();
                },
                on:wheel = move |ev| {
                    if let Some(spot) = Spot::locate(track, ev.position.x.0, ev.position.y.0) {
                        let delta = ev.delta.to_pixels(CssPx(LINE));
                        let held = ev.modifiers;
                        wheel.wheel(
                            spot,
                            f64::from(delta.width.0),
                            f64::from(delta.height.0),
                            held.primary() || held.control() || held.alt(),
                        );
                    }
                }
            ) {
                box(class = "ruler") {{move || ruler::labels(viewport.get(), track_width.get())}}
                box(class = "lane") {}
                {marks.into_view()}
                {playhead.into_view()}
            }
        }
    }
}

/// One of the small transport buttons: an icon named `label`, in the
/// style sheet's `class`, running `action` when pressed.
fn transport_button(
    icon: IconData,
    label: &'static str,
    class: &'static str,
    action: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        Button(
            variant = ButtonVariant::Ghost,
            size = ButtonSize::IconSm,
            class = class,
            on:click = move |_| action()
        ) {
            Icon(icon = icon, label = label)
        }
    }
}
