//! The timeline under the preview: transport buttons and timecode above a
//! ruler and lane with a draggable playhead.

use std::time::Duration;

use vello_winit_motiongfx::PlaybackState;
use zgui::canvas::zgui_color::Color;
use zgui::canvas::{Brush, ShapeBuilder};
use zgui::elements::kurbo::{Rect, RoundedRect, Shape as _};
use zgui::prelude::*;
use zgui::view::CustomPropertyName;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

use crate::editor::Clock;

/// The timeline's height in logical pixels, added below the canvas when
/// sizing the window.
pub(crate) const HEIGHT: f64 = 104.0;

/// The frame rate the timecode and frame steps count in.
const FPS: u32 = 60;

/// One frame at [`FPS`].
pub(crate) const FRAME: Duration =
    Duration::from_nanos(1_000_000_000 / FPS as u64);

/// Where the lane starts below the track's top, and ends above its
/// bottom; the stylesheet's `.lane` uses the same numbers.
const LANE_TOP: f64 = 30.0;
const LANE_BOTTOM: f64 = 12.0;

/// Where the playhead's knob sits below the track's top.
const HEAD_TOP: f64 = 12.0;

/// The timeline for `clock`.
pub(crate) fn timeline(clock: Clock) -> impl IntoView {
    let state = clock.state;
    let duration = state.get_untracked().duration;

    // The playhead and the played part of the lane are one vector drawing
    // rather than positioned boxes: boxes snap to whole device pixels one
    // by one, so the line and its knob would step and drift apart by a
    // pixel as they move.
    let playhead =
        zgui::elements::canvas().class("playhead").draw(move |cx| {
            let state = state.get();
            let fraction = (state.time.as_secs_f64()
                / state.duration.as_secs_f64())
            .clamp(0.0, 1.0);
            let (width, height) = (
                f64::from(cx.size.width.0),
                f64::from(cx.size.height.0),
            );
            let x = fraction * width;
            let played =
                Rect::new(0.0, LANE_TOP, x, height - LANE_BOTTOM);
            if played.width() > 0.0 {
                cx.scene.push(
                    ShapeBuilder::new(
                        RoundedRect::from_rect(played, 4.0)
                            .to_path(0.1),
                    )
                    .fill(Brush::Solid(Color::srgb(
                        0.95, 0.72, 0.29, 0.16,
                    )))
                    .build(),
                );
            }
            let mut head =
                Rect::new(x - 1.0, HEAD_TOP, x + 1.0, height - 6.0)
                    .to_path(0.1);
            head.extend(
                RoundedRect::new(
                    x - 6.0,
                    HEAD_TOP,
                    x + 6.0,
                    HEAD_TOP + 12.0,
                    3.0,
                )
                .path_elements(0.1),
            );
            cx.scene.push(
                ShapeBuilder::new(head)
                    .fill(Brush::Solid(Color::srgb(
                        0.95, 0.72, 0.29, 1.0,
                    )))
                    .build(),
            );
        });

    // Pointer positions arrive in window CSS pixels and boxes in device
    // pixels, so the track's window box and the scale relate them.
    let track = NodeRef::new();
    let dragging = RwSignal::new_local(false);
    let scrub = clock.clone();
    let seek_to = move |x_css: f32| {
        let Some(bounds) = track.window_bounds() else {
            return;
        };
        if bounds.size.width.0 <= 0.0 {
            return;
        }
        let x = x_css * track.scale();
        let fraction =
            f64::from((x - bounds.origin.x.0) / bounds.size.width.0);
        scrub.seek(duration.mul_f64(fraction.clamp(0.0, 1.0)));
    };
    let seek_down = seek_to.clone();

    let (start, back, play, forward, end) = (
        clock.clone(),
        clock.clone(),
        clock.clone(),
        clock.clone(),
        clock,
    );

    view! {
        column(class = "timeline") {
            row(class = "transport") {
                row(class = "transport__buttons") {
                    Button(
                        variant = ButtonVariant::Ghost,
                        size = ButtonSize::IconSm,
                        on:click = move |_| start.seek(Duration::ZERO)
                    ) {
                        Icon(icon = TO_START, label = "Go to start")
                    }
                    Button(
                        variant = ButtonVariant::Ghost,
                        size = ButtonSize::IconSm,
                        on:click = move |_| back.step(-1)
                    ) {
                        Icon(icon = FRAME_BACK, label = "Previous frame")
                    }
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
                    Button(
                        variant = ButtonVariant::Ghost,
                        size = ButtonSize::IconSm,
                        on:click = move |_| forward.step(1)
                    ) {
                        Icon(icon = FRAME_FORWARD, label = "Next frame")
                    }
                    Button(
                        variant = ButtonVariant::Ghost,
                        size = ButtonSize::IconSm,
                        on:click = move |_| end.seek(duration)
                    ) {
                        Icon(icon = TO_END, label = "Go to end")
                    }
                }
                text(class = "timecode") {{move || timecode(state.get().time)}}
                text(class = "timecode timecode--total") {{format!("/ {}", timecode(duration))}}
                box(class = "transport__spacer")
                text(class = "transport__meta") {{move || frame_text(state.get())}}
            }
            box(
                class = "track",
                node_ref = track,
                on:pointer_down = move |ev| {
                    ev.capture_pointer();
                    dragging.set(true);
                    seek_down(ev.position.x.0);
                },
                on:pointer_move = move |ev| {
                    if dragging.get_untracked() {
                        seek_to(ev.position.x.0);
                    }
                },
                on:pointer_up = move |ev| {
                    dragging.set(false);
                    ev.release_pointer();
                },
                on:pointer_cancel = move |ev| {
                    dragging.set(false);
                    ev.release_pointer();
                }
            ) {
                box(class = "ruler") {{ruler(duration)}}
                box(class = "lane") {}
                {playhead.into_view()}
            }
        }
    }
}

/// The ruler's ticks: a labelled major tick every `major` seconds and
/// minor ticks between, spaced so a short scene and a long one both read.
fn ruler(duration: Duration) -> Vec<impl View> {
    let seconds = duration.as_secs_f64();
    let major =
        [0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0]
            .into_iter()
            .find(|step| seconds / step <= 12.0)
            .unwrap_or(600.0);
    let minor = major / 5.0;
    let count = (seconds / minor).floor() as u32;
    (0..=count)
        .map(|index| {
            let at = f64::from(index) * minor;
            let is_major = index % 5 == 0;
            let place = Attrs::new().custom_property(
                property("mg-at"),
                Some(format!("{:.4}%", at / seconds * 100.0)),
            );
            let label = is_major.then(|| ruler_label(at));
            view! {
                box(
                    class = if is_major { "tick tick--major" } else { "tick" },
                    {..place}
                ) {
                    {label.map(|label| view! { text(class = "tick__label") {{label}} })}
                }
            }
            .into_view()
        })
        .collect()
}

/// A ruler label: whole seconds as `3s`, others to the tenth.
fn ruler_label(seconds: f64) -> String {
    if seconds.fract() == 0.0 {
        format!("{seconds:.0}s")
    } else {
        format!("{seconds:.1}s")
    }
}

/// `MM:SS:FF`, with frames at [`FPS`].
fn timecode(time: Duration) -> String {
    let frames = frame_of(time);
    let fps = u64::from(FPS);
    let (minutes, rest) = (frames / (60 * fps), frames % (60 * fps));
    format!("{minutes:02}:{:02}:{:02}", rest / fps, rest % fps)
}

/// `frame 74 / 300`.
fn frame_text(state: PlaybackState) -> String {
    format!(
        "frame {} / {}  ·  {FPS} fps",
        frame_of(state.time),
        frame_of(state.duration)
    )
}

/// The frame `time` falls in. Rounded, so a seek to a frame's start
/// shows that frame despite float error.
fn frame_of(time: Duration) -> u64 {
    (time.as_secs_f64() * f64::from(FPS)).round() as u64
}

fn property(name: &'static str) -> CustomPropertyName {
    CustomPropertyName::new(name)
}

const PLAY: IconData =
    IconData::new("play", 24.0, "M8 5.5 L19 12 L8 18.5 Z");
const PAUSE: IconData = IconData::new(
    "pause",
    24.0,
    "M6.5 5 H10 V19 H6.5 Z M14 5 H17.5 V19 H14 Z",
);
const TO_START: IconData = IconData::new(
    "to-start",
    24.0,
    "M5 5 H7.5 V19 H5 Z M19 5.5 L9 12 L19 18.5 Z",
);
const TO_END: IconData = IconData::new(
    "to-end",
    24.0,
    "M16.5 5 H19 V19 H16.5 Z M5 5.5 L15 12 L5 18.5 Z",
);
const FRAME_BACK: IconData = IconData::new(
    "frame-back",
    24.0,
    "M15.5 6 L9.5 12 L15.5 18 L17 16.5 L12.5 12 L17 7.5 Z M7 6 H9 V18 H7 Z",
);
const FRAME_FORWARD: IconData = IconData::new(
    "frame-forward",
    24.0,
    "M8.5 6 L14.5 12 L8.5 18 L7 16.5 L11.5 12 L7 7.5 Z M15 6 H17 V18 H15 Z",
);

/// How the timeline looks.
pub(crate) const SHEET: &str = zgui::css!(
    ".timeline {
        height: 104px;
        flex-shrink: 0;
        background-color: #15171c;
        border-top: 1px solid #262a33;
    }

    .transport {
        height: 44px;
        flex-shrink: 0;
        align-items: center;
        gap: 10px;
        padding: 0 12px;
        border-bottom: 1px solid #22252d;
    }

    .transport__buttons {
        align-items: center;
        gap: 2px;
    }

    .transport__play {
        margin: 0 4px;
    }

    .timecode {
        font-size: 17px;
        font-variant-numeric: tabular-nums;
        color: #f1f3f7;
        margin-left: 8px;
    }

    .timecode--total {
        font-size: 13px;
        color: #6c7383;
        margin-left: 0;
    }

    .transport__spacer {
        flex-grow: 1;
    }

    .transport__meta {
        font-size: 12px;
        font-variant-numeric: tabular-nums;
        color: #6c7383;
    }

    .track {
        position: relative;
        flex-grow: 1;
        margin: 0 20px;
    }

    .ruler {
        position: absolute;
        left: 0;
        right: 0;
        top: 0;
        height: 26px;
    }

    .tick {
        position: absolute;
        left: var(--mg-at, 0%);
        bottom: 0;
        width: 1px;
        height: 5px;
        background-color: #353a45;
    }

    .tick--major {
        height: 11px;
        background-color: #596071;
    }

    .tick__label {
        position: absolute;
        left: 4px;
        bottom: 3px;
        font-size: 10px;
        font-variant-numeric: tabular-nums;
        color: #7d8597;
    }

    .lane {
        position: absolute;
        left: 0;
        right: 0;
        top: 30px; /* LANE_TOP */
        bottom: 12px; /* LANE_BOTTOM */
        border-radius: 4px;
        background-color: #1e2128;
    }

    .playhead {
        position: absolute;
        left: 0;
        right: 0;
        top: 0;
        bottom: 0;
    }"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_counts_minutes_seconds_and_frames() {
        assert_eq!(timecode(Duration::ZERO), "00:00:00");
        assert_eq!(
            timecode(Duration::from_secs_f64(61.5)),
            "01:01:30"
        );
        // The last frame of a second, not the next second.
        assert_eq!(timecode(FRAME * 59), "00:00:59");
    }

    #[test]
    fn stepping_frames_lands_on_whole_frames() {
        assert_eq!(frame_of(FRAME * 7), 7);
        assert_eq!(frame_of(FRAME * 7 + Duration::from_nanos(3)), 7);
    }

    #[test]
    fn rulers_label_whole_and_fractional_seconds() {
        assert_eq!(ruler_label(3.0), "3s");
        assert_eq!(ruler_label(2.5), "2.5s");
    }
}
