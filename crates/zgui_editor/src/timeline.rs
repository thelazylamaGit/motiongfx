//! The timeline under the preview: transport buttons and timecode above a
//! ruler and lane with a draggable playhead. The track zooms from the
//! whole animation down to a few frames.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use vello_winit_motiongfx::PlaybackState;
use zgui::canvas::zgui_color::Color;
use zgui::canvas::{Brush, ShapeBuilder};
use zgui::elements::kurbo::{BezPath, Rect, RoundedRect, Shape as _};
use zgui::geom::CssPx;
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui::view::{CustomPropertyName, FrameHandle};
use zgui::vocab::Timestamp;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

use crate::controls::{Sound, speed_menu, volume_control};
use crate::editor::Clock;

/// The timeline's height in logical pixels, added below the canvas when
/// sizing the window.
pub(crate) const HEIGHT: f64 = 104.0;

/// The frame rate the timecode, ruler and frame steps count in.
const FPS: u32 = 60;

/// One frame at [`FPS`].
pub(crate) const FRAME: Duration =
    Duration::from_nanos(1_000_000_000 / FPS as u64);

/// The ruler's height; ticks stand on its bottom edge.
const RULER: f64 = 26.0;

/// Where the lane starts below the track's top, and ends above its
/// bottom; the stylesheet's `.lane` uses the same numbers.
const LANE_TOP: f64 = 30.0;
const LANE_BOTTOM: f64 = 12.0;

/// Where the playhead's knob sits below the track's top.
const HEAD_TOP: f64 = 12.0;

/// The navigator bar under the lane, shown while zoomed: its top above
/// the track's bottom, and its height.
const NAV_RAISE: f64 = 8.0;
const NAV_HEIGHT: f64 = 4.0;

/// The narrowest the navigator's thumb gets, so it can still be grabbed.
const THUMB_MIN: f64 = 16.0;

/// How much `=` and `-` zoom per press.
pub(crate) const ZOOM_STEP: f64 = 1.5;

/// How much one CSS pixel of wheel zooms, as an exponent, so zooming in
/// and back out by the same amount returns to the same view.
const WHEEL_ZOOM: f64 = 0.004;

/// CSS pixels per wheel notch, for wheels that report notches.
const LINE: f32 = 40.0;

/// The fewest frames the track can be zoomed to show.
const MIN_FRAMES: u32 = 4;

/// Frame stripes appear on the lane once a frame is this many CSS pixels
/// wide.
const STRIPE_MIN: f64 = 8.0;

/// The room each ruler label is given, in CSS pixels.
const LABEL_GAP: f64 = 72.0;

/// How fast the track scrolls while the playhead is dragged past its
/// edge, in CSS pixels per second: a base, plus this much per pixel the
/// pointer is past the edge, up to a limit. Further out scrolls faster,
/// as in most editors.
const EDGE_BASE: f64 = 150.0;
const EDGE_GAIN: f64 = 12.0;
const EDGE_MAX: f64 = 2400.0;

/// The stretch of the animation the track shows, in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Viewport {
    start: f64,
    length: f64,
    duration: f64,
}

impl Viewport {
    /// The whole of an animation `duration` long.
    pub(crate) fn whole(duration: Duration) -> Self {
        let duration = duration.as_secs_f64();
        Self {
            start: 0.0,
            length: duration,
            duration,
        }
    }

    #[cfg(test)]
    pub(crate) fn start(self) -> f64 {
        self.start
    }

    #[cfg(test)]
    pub(crate) fn length(self) -> f64 {
        self.length
    }

    pub(crate) fn end(self) -> f64 {
        self.start + self.length
    }

    fn is_zoomed(self) -> bool {
        self.length < self.duration
    }

    /// The time at `fraction` of the track's width.
    pub(crate) fn time_at(self, fraction: f64) -> f64 {
        self.start + fraction * self.length
    }

    /// Where `seconds` falls, as a fraction of the track's width.
    fn fraction_of(self, seconds: f64) -> f64 {
        (seconds - self.start) / self.length
    }

    fn starting_at(self, start: f64) -> Self {
        Self {
            start: start.clamp(0.0, self.duration - self.length),
            ..self
        }
    }

    /// Zoomed in by `factor` (out, below one), keeping the time at
    /// `anchor` (a fraction of the width) where it is.
    pub(crate) fn zoomed(self, factor: f64, anchor: f64) -> Self {
        let pinned = self.time_at(anchor);
        let shortest =
            (FRAME * MIN_FRAMES).as_secs_f64().min(self.duration);
        let length =
            (self.length / factor).clamp(shortest, self.duration);
        Self { length, ..self }.starting_at(pinned - anchor * length)
    }

    /// Zoomed by `factor` around `seconds`: in place when it is in view,
    /// otherwise centred on it.
    pub(crate) fn zoomed_around(
        self,
        factor: f64,
        seconds: f64,
    ) -> Self {
        let anchor = self.fraction_of(seconds);
        if (0.0..=1.0).contains(&anchor) {
            self.zoomed(factor, anchor)
        } else {
            let zoomed = self.zoomed(factor, 0.5);
            zoomed.starting_at(seconds - zoomed.length / 2.0)
        }
    }

    /// The whole animation.
    pub(crate) fn fitted(self) -> Self {
        Self {
            start: 0.0,
            length: self.duration,
            ..self
        }
    }

    fn panned(self, seconds: f64) -> Self {
        self.starting_at(self.start + seconds)
    }

    /// Turned a page, if need be, so `seconds` is in view: how an editor
    /// keeps up with the playhead.
    fn following(self, seconds: f64) -> Self {
        // The clock rounds time to the nanosecond, so a seek to an edge
        // can land a hair outside it; that is still on this page.
        const SLACK: f64 = 1e-6;
        if seconds > self.end() + SLACK {
            self.starting_at(seconds)
        } else if seconds < self.start - SLACK {
            self.starting_at(seconds - self.length)
        } else {
            self
        }
    }
}

/// What a press on the track started.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Idle,
    /// Moving the playhead.
    Scrub,
    /// Moving the navigator's thumb, held `grab` seconds after the
    /// view's start.
    Navigate {
        grab: f64,
    },
}

/// Scrolls the track, frame by frame, while the playhead is dragged past
/// either end of a zoomed view. The scroll is paced by time, not by how
/// often the pointer reports, so it is smooth and doesn't run away.
#[derive(Clone)]
struct EdgeScroll {
    /// How far past the edge the pointer is, in CSS pixels: negative
    /// before the start, zero inside.
    overshoot: Rc<Cell<f64>>,
    /// The track's width in CSS pixels.
    width: Rc<Cell<f64>>,
    last: Rc<Cell<Option<Timestamp>>>,
    frame: Rc<RefCell<Option<FrameHandle>>>,
    timers: Timers,
}

impl EdgeScroll {
    fn new() -> Self {
        Self {
            overshoot: Rc::default(),
            width: Rc::default(),
            last: Rc::default(),
            frame: Rc::default(),
            timers: Timers::current()
                .expect("the timeline is built in a window"),
        }
    }

    /// The pointer is `overshoot` pixels past an edge of a track `width`
    /// wide; start scrolling if it has just gone past.
    fn reach(
        &self,
        overshoot: f64,
        width: f64,
        viewport: RwSignal<Viewport, LocalStorage>,
        clock: &Clock,
    ) {
        self.width.set(width);
        let was = self.overshoot.replace(overshoot);
        if overshoot != 0.0 && was == 0.0 {
            self.last.set(None);
            self.arm(viewport, clock.clone());
        } else if overshoot == 0.0 {
            self.stop();
        }
    }

    fn stop(&self) {
        self.overshoot.set(0.0);
        self.frame.borrow_mut().take();
    }

    fn arm(
        &self,
        viewport: RwSignal<Viewport, LocalStorage>,
        clock: Clock,
    ) {
        let edge = self.clone();
        let next = self.timers.request_frame(move |at| {
            if edge.step(at, viewport, &clock) {
                edge.arm(viewport, clock);
            }
        });
        *self.frame.borrow_mut() = Some(next);
    }

    /// Scroll for the time since the last frame and keep the playhead on
    /// the edge; whether to keep going.
    fn step(
        &self,
        at: Timestamp,
        viewport: RwSignal<Viewport, LocalStorage>,
        clock: &Clock,
    ) -> bool {
        let overshoot = self.overshoot.get();
        let width = self.width.get();
        if overshoot == 0.0 || width <= 0.0 {
            return false;
        }
        // A long gap (a stall, a window drag) shouldn't jump the view.
        let elapsed =
            self.last.replace(Some(at)).map_or(0.0, |last| {
                at.saturating_since(last).as_secs_f64().min(0.05)
            });
        let view = viewport.get_untracked();
        let pixels = (EDGE_BASE + overshoot.abs() * EDGE_GAIN)
            .min(EDGE_MAX)
            * elapsed;
        let next = view.panned(
            overshoot.signum() * pixels / width * view.length,
        );
        if next != view {
            viewport.set(next);
        }
        let edge = if overshoot > 0.0 {
            next.end()
        } else {
            next.start
        };
        clock.seek(Duration::from_secs_f64(edge));
        true
    }
}

/// Where a pointer is over the track.
#[derive(Clone, Copy, Debug)]
struct Spot {
    /// Across the track, as a fraction of its width.
    fraction: f64,
    /// Down from the track's top, in CSS pixels.
    y: f64,
    /// The track's size, in CSS pixels.
    width: f64,
    height: f64,
}

/// The timeline for `clock`, showing `viewport`, with the volume control
/// for `sound`.
pub(crate) fn timeline(
    clock: Clock,
    viewport: RwSignal<Viewport, LocalStorage>,
    sound: Sound,
) -> impl IntoView {
    let state = clock.state;
    let duration = state.get_untracked().duration;
    // The track's width, which decides how many ruler labels fit.
    let track_width = RwSignal::new_local(0.0);

    // Turn the page when the playhead leaves the view. Only the time is
    // tracked, so panning away while paused stays put.
    let drag = Rc::new(Cell::new(Drag::Idle));
    // While a drag is on, it decides what is in view.
    let dragging = drag.clone();
    let follow = RenderEffect::new(move |_| {
        let time = state.get().time.as_secs_f64();
        if dragging.get() != Drag::Idle {
            return;
        }
        let now = viewport.get_untracked();
        let next = now.following(time);
        if next != now {
            viewport.set(next);
        }
    });
    on_cleanup_local(move || drop(follow));

    // What moves only with the view: the ruler's ticks, frame stripes and
    // the navigator. A canvas can't draw text, so labels are elements.
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
            draw_ticks(cx, view, width);
            draw_stripes(cx, view, width, height);
            if view.is_zoomed() {
                draw_navigator(cx, view, width, height);
            }
        });

    // The playhead and the played part of the lane are one vector drawing
    // rather than positioned boxes: boxes snap to whole device pixels one
    // by one, so the line and its knob would step and drift apart by a
    // pixel as they move.
    let playhead =
        zgui::elements::canvas().class("playhead").draw(move |cx| {
            let (width, height) = (
                f64::from(cx.size.width.0),
                f64::from(cx.size.height.0),
            );
            if width <= 0.0 {
                return;
            }
            draw_playhead(
                cx,
                viewport.get(),
                state.get(),
                width,
                height,
            );
        });

    // Pointer positions arrive in window CSS pixels and boxes in device
    // pixels, so the track's window box and the scale relate them.
    let track = NodeRef::new();
    #[cfg(test)]
    crate::headless::TRACK.set(Some(track));
    let locate = move |x_css: f32, y_css: f32| {
        let bounds = track.window_bounds()?;
        let scale = track.scale();
        if bounds.size.width.0 <= 0.0 || scale <= 0.0 {
            return None;
        }
        Some(Spot {
            fraction: f64::from(
                (x_css * scale - bounds.origin.x.0)
                    / bounds.size.width.0,
            ),
            y: f64::from((y_css * scale - bounds.origin.y.0) / scale),
            width: f64::from(bounds.size.width.0 / scale),
            height: f64::from(bounds.size.height.0 / scale),
        })
    };

    let scrub = clock.clone();
    let edge = EdgeScroll::new();
    // Inside the track the playhead follows the pointer. Past either end
    // of a zoomed view it waits on the edge while the track scrolls under
    // it, at a pace set by how far past the pointer is.
    let scrub_to = {
        let edge = edge.clone();
        move |spot: Spot| {
            let view = viewport.get_untracked();
            let overshoot = if !view.is_zoomed() {
                0.0
            } else if spot.fraction < 0.0 {
                spot.fraction * spot.width
            } else {
                (spot.fraction - 1.0).max(0.0) * spot.width
            };
            if overshoot == 0.0 {
                let time =
                    view.time_at(spot.fraction.clamp(0.0, 1.0));
                scrub.seek(Duration::from_secs_f64(time));
            }
            edge.reach(overshoot, spot.width, viewport, &scrub);
        }
    };
    // The navigator spans the whole animation, so the pointer's fraction
    // of the width is a fraction of the duration.
    let navigate_to = move |fraction: f64, grab: f64| {
        let view = viewport.get_untracked();
        let next = view.starting_at(fraction * view.duration - grab);
        if next != view {
            viewport.set(next);
        }
    };
    let press = {
        let drag = drag.clone();
        let scrub_to = scrub_to.clone();
        move |spot: Spot| {
            let view = viewport.get_untracked();
            if view.is_zoomed() && spot.y >= spot.height - LANE_BOTTOM
            {
                // Grabbing the thumb keeps it under the pointer; pressing
                // beside it centres it there first.
                let at = spot.fraction * view.duration;
                let grab = if (view.start..=view.end()).contains(&at)
                {
                    at - view.start
                } else {
                    view.length / 2.0
                };
                drag.set(Drag::Navigate { grab });
                navigate_to(spot.fraction, grab);
            } else {
                drag.set(Drag::Scrub);
                scrub_to(spot);
            }
        }
    };
    let drag_to = {
        let drag = drag.clone();
        move |spot: Spot| match drag.get() {
            Drag::Idle => {}
            Drag::Scrub => scrub_to(spot),
            Drag::Navigate { grab } => {
                navigate_to(spot.fraction, grab)
            }
        }
    };
    let release = {
        let edge = edge.clone();
        move || {
            drag.set(Drag::Idle);
            edge.stop();
        }
    };
    let cancel = release.clone();
    // Ctrl (Cmd on macOS) or Alt with the wheel zooms around the pointer,
    // as in most editors; the plain wheel scrolls through time whichever
    // way it turns.
    let wheel = move |spot: Spot, dx: f64, dy: f64, zoom: bool| {
        let view = viewport.get_untracked();
        let next = if zoom {
            view.zoomed(
                (-dy * WHEEL_ZOOM).exp(),
                spot.fraction.clamp(0.0, 1.0),
            )
        } else {
            view.panned((dx + dy) / spot.width * view.length)
        };
        if next != view {
            viewport.set(next);
        }
    };

    let (start, back, play, forward, end, speed) = (
        clock.clone(),
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
                        class = "transport__jump",
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
                        class = "transport__jump",
                        on:click = move |_| end.seek(duration)
                    ) {
                        Icon(icon = TO_END, label = "Go to end")
                    }
                }
                text(class = "timecode") {{move || timecode(state.get().time)}}
                text(class = "timecode timecode--total") {{format!("/ {}", timecode(duration))}}
                box(class = "transport__spacer")
                text(class = "transport__meta") {{move || meta_text(state.get(), viewport.get())}}
                row(class = "transport__controls") {
                    {speed_menu(speed)}
                    {volume_control(sound)}
                }
            }
            box(
                class = "track",
                node_ref = track,
                on:pointer_down = move |ev| {
                    if let Some(spot) = locate(ev.position.x.0, ev.position.y.0) {
                        ev.capture_pointer();
                        press(spot);
                    }
                },
                on:pointer_move = move |ev| {
                    if let Some(spot) = locate(ev.position.x.0, ev.position.y.0) {
                        drag_to(spot);
                    }
                },
                on:pointer_up = move |ev| {
                    release();
                    ev.release_pointer();
                },
                on:pointer_cancel = move |ev| {
                    cancel();
                    ev.release_pointer();
                },
                on:wheel = move |ev| {
                    if let Some(spot) = locate(ev.position.x.0, ev.position.y.0) {
                        let delta = ev.delta.to_pixels(CssPx(LINE));
                        let modifiers = ev.modifiers;
                        wheel(
                            spot,
                            f64::from(delta.width.0),
                            f64::from(delta.height.0),
                            modifiers.primary() || modifiers.control() || modifiers.alt(),
                        );
                    }
                }
            ) {
                box(class = "ruler") {{move || labels(viewport.get(), track_width.get())}}
                box(class = "lane") {}
                {marks.into_view()}
                {playhead.into_view()}
            }
        }
    }
}

fn fill(cx: &mut DrawCx<'_>, path: BezPath, color: Color) {
    if !path.is_empty() {
        cx.scene.push(
            ShapeBuilder::new(path).fill(Brush::Solid(color)).build(),
        );
    }
}

/// The ruler's ticks along its bottom edge, each one device pixel wide so
/// they stay sharp at any display scale.
fn draw_ticks(cx: &mut DrawCx<'_>, view: Viewport, width: f64) {
    let scale = f64::from(cx.scale).max(1.0);
    let (major, divisions) = ruler_step(view.length, width);
    let minor = major / f64::from(divisions);
    let mut majors = BezPath::new();
    let mut minors = BezPath::new();
    let first = (view.start / minor).floor() as i64;
    let last = (view.end() / minor).ceil() as i64;
    for index in first..=last {
        let at = index as f64 * minor;
        if at > view.duration + 1e-9 {
            break;
        }
        let x =
            (view.fraction_of(at) * width * scale).round() / scale;
        if !(0.0..=width).contains(&x) {
            continue;
        }
        let is_major = index.rem_euclid(i64::from(divisions)) == 0;
        let (path, height) = if is_major {
            (&mut majors, 11.0)
        } else {
            (&mut minors, 5.0)
        };
        path.extend(
            Rect::new(x, RULER - height, x + 1.0 / scale, RULER)
                .path_elements(0.1),
        );
    }
    fill(cx, minors, Color::srgb(0.21, 0.23, 0.27, 1.0));
    fill(cx, majors, Color::srgb(0.35, 0.38, 0.44, 1.0));
}

/// Every other frame shaded on the lane, once frames are wide enough to
/// tell apart.
fn draw_stripes(
    cx: &mut DrawCx<'_>,
    view: Viewport,
    width: f64,
    height: f64,
) {
    let frame = FRAME.as_secs_f64();
    if frame / view.length * width < STRIPE_MIN {
        return;
    }
    let mut stripes = BezPath::new();
    let first = (view.start / frame).floor() as i64;
    let last = (view.end() / frame).ceil() as i64;
    for index in
        (first..=last).filter(|index| index.rem_euclid(2) == 1)
    {
        let at = index as f64 * frame;
        let left = (view.fraction_of(at) * width).max(0.0);
        let right = (view.fraction_of(at + frame) * width).min(width);
        if right > left {
            stripes.extend(
                Rect::new(
                    left,
                    LANE_TOP,
                    right,
                    height - LANE_BOTTOM,
                )
                .path_elements(0.1),
            );
        }
    }
    fill(cx, stripes, Color::srgb(1.0, 1.0, 1.0, 0.03));
}

/// The navigator: the whole animation as a bar, with the view as a thumb
/// to drag.
fn draw_navigator(
    cx: &mut DrawCx<'_>,
    view: Viewport,
    width: f64,
    height: f64,
) {
    let top = height - NAV_RAISE;
    let bar =
        RoundedRect::new(0.0, top, width, top + NAV_HEIGHT, 2.0);
    fill(cx, bar.to_path(0.1), Color::srgb(0.13, 0.15, 0.18, 1.0));
    let mut left = view.start / view.duration * width;
    let mut right = view.end() / view.duration * width;
    let shortfall = THUMB_MIN - (right - left);
    if shortfall > 0.0 {
        left = (left - shortfall / 2.0)
            .clamp(0.0, (width - THUMB_MIN).max(0.0));
        right = left + THUMB_MIN;
    }
    let thumb =
        RoundedRect::new(left, top, right, top + NAV_HEIGHT, 2.0);
    fill(cx, thumb.to_path(0.1), Color::srgb(0.35, 0.38, 0.44, 1.0));
}

/// The played part of the lane and the playhead; while zoomed, also the
/// playhead's place on the navigator.
fn draw_playhead(
    cx: &mut DrawCx<'_>,
    view: Viewport,
    state: PlaybackState,
    width: f64,
    height: f64,
) {
    let time = state.time.as_secs_f64();
    let x = view.fraction_of(time) * width;
    let played = Rect::new(
        0.0,
        LANE_TOP,
        x.clamp(0.0, width),
        height - LANE_BOTTOM,
    );
    if played.width() > 0.0 {
        fill(
            cx,
            RoundedRect::from_rect(played, 4.0).to_path(0.1),
            Color::srgb(0.95, 0.72, 0.29, 0.16),
        );
    }
    let mut head = BezPath::new();
    // The knob may overhang the track's ends by half its width, so a
    // playhead at the very start or end isn't cut in half.
    if (-6.0..=width + 6.0).contains(&x) {
        head.extend(
            Rect::new(
                x - 1.0,
                HEAD_TOP,
                x + 1.0,
                height - LANE_BOTTOM,
            )
            .path_elements(0.1),
        );
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
    }
    if view.is_zoomed() {
        let at = time / view.duration * width;
        let top = height - NAV_RAISE;
        head.extend(
            Rect::new(
                at - 1.0,
                top - 1.0,
                at + 1.0,
                top + NAV_HEIGHT + 1.0,
            )
            .path_elements(0.1),
        );
    }
    fill(cx, head, Color::srgb(0.95, 0.72, 0.29, 1.0));
}

/// The ruler's step between labelled ticks and how many parts it splits
/// into, for `length` seconds across `width` CSS pixels: the finest that
/// gives each label [`LABEL_GAP`] of room, so the ruler reads the same at
/// every zoom and window size. Before the first layout, when the width is
/// still zero, it allows ten labels.
fn ruler_step(length: f64, width: f64) -> (f64, u32) {
    const F: f64 = 1.0 / FPS as f64;
    const STEPS: [(f64, u32); 16] = [
        (F, 1),
        (2.0 * F, 2),
        (5.0 * F, 5),
        (10.0 * F, 2),
        (15.0 * F, 3),
        (30.0 * F, 2),
        (1.0, 4),
        (2.0, 4),
        (5.0, 5),
        (10.0, 5),
        (15.0, 3),
        (30.0, 6),
        (60.0, 6),
        (120.0, 4),
        (300.0, 5),
        (600.0, 5),
    ];
    let most = if width > 0.0 {
        (width / LABEL_GAP).max(1.0)
    } else {
        10.0
    };
    STEPS
        .into_iter()
        .find(|(major, _)| length / major <= most)
        .unwrap_or((1200.0, 4))
}

/// The labels of the major ticks in `view`, across a track `width` CSS
/// pixels wide.
fn labels(view: Viewport, width: f64) -> Vec<impl View> {
    let (major, _) = ruler_step(view.length, width);
    let first = (view.start / major - 1e-9).ceil() as i64;
    let last = (view.end() / major + 1e-9).floor() as i64;
    (first..=last)
        .filter_map(|index| {
            let at = index as f64 * major;
            let fraction = view.fraction_of(at);
            // The last label would hang off the track's end.
            (fraction < 0.98).then(|| {
                let place = Attrs::new().custom_property(
                    property("mg-at"),
                    Some(format!("{:.4}%", fraction * 100.0)),
                );
                view! {
                    text(class = "tick__label", {..place}) {{ruler_label(at)}}
                }
                .into_view()
            })
        })
        .collect()
}

/// A ruler label: `3s`, `1m05s`, or with frames, `3s 15f` or `15f`.
fn ruler_label(seconds: f64) -> String {
    let frames = (seconds * f64::from(FPS)).round() as u64;
    let fps = u64::from(FPS);
    let (whole, frame) = (frames / fps, frames % fps);
    let base = if whole >= 60 {
        format!("{}m{:02}s", whole / 60, whole % 60)
    } else {
        format!("{whole}s")
    };
    match (whole, frame) {
        (_, 0) => base,
        (0, frame) => format!("{frame}f"),
        (_, frame) => format!("{base} {frame}f"),
    }
}

/// `MM:SS:FF`, with frames at [`FPS`].
fn timecode(time: Duration) -> String {
    let frames = frame_of(time);
    let fps = u64::from(FPS);
    let (minutes, rest) = (frames / (60 * fps), frames % (60 * fps));
    format!("{minutes:02}:{:02}:{:02}", rest / fps, rest % fps)
}

/// `frame 74 / 300  ·  60 fps`, led by the zoom while zoomed in.
fn meta_text(state: PlaybackState, view: Viewport) -> String {
    let frames = format!(
        "frame {} / {}  ·  {FPS} fps",
        frame_of(state.time),
        frame_of(state.duration)
    );
    if view.is_zoomed() {
        format!("{:.1}×  ·  {frames}", view.duration / view.length)
    } else {
        frames
    }
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
        overflow: hidden;
    }

    /* Nothing in the bar wraps: what doesn't fit is shortened or, below
       the widths at the end of this sheet, left out, least needed first. */
    .transport__buttons, .transport__controls {
        flex-shrink: 0;
        align-items: center;
        gap: 2px;
    }

    .transport__play {
        margin: 0 4px;
    }

    .timecode {
        flex-shrink: 0;
        white-space: nowrap;
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
        min-width: 0;
    }

    .transport__meta {
        min-width: 0;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
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
        height: 26px; /* RULER */
        overflow: hidden;
    }

    .tick__label {
        position: absolute;
        left: var(--mg-at, 0%);
        margin-left: 4px;
        bottom: 6px;
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

    /* A canvas keeps its default 300 x 150 unless it is given a size, so
       insets alone would not stretch it over the track. */
    .marks, .playhead {
        position: absolute;
        left: 0;
        top: 0;
        width: 100%;
        height: 100%;
    }

    @media (max-width: 720px) {
        .transport__meta { display: none; }
    }

    @media (max-width: 560px) {
        .timecode--total { display: none; }
    }

    @media (max-width: 440px) {
        .transport .transport__jump { display: none; }
        .transport { gap: 6px; padding: 0 8px; }
        .track { margin: 0 12px; }
    }"
);

#[cfg(test)]
mod tests {
    use super::*;

    fn ten_seconds() -> Viewport {
        Viewport::whole(Duration::from_secs(10))
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

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
    fn rulers_label_seconds_minutes_and_frames() {
        assert_eq!(ruler_label(3.0), "3s");
        assert_eq!(ruler_label(65.0), "1m05s");
        assert_eq!(ruler_label(2.5), "2s 30f");
        assert_eq!(ruler_label(0.25), "15f");
    }

    #[test]
    fn zooming_keeps_the_time_under_the_pointer() {
        let view = ten_seconds().zoomed(4.0, 0.3);
        assert!(close(view.length, 2.5));
        assert!(close(view.time_at(0.3), 3.0));
        let back = view.zoomed(0.25, 0.3);
        assert!(close(back.start, 0.0) && close(back.length, 10.0));
    }

    #[test]
    fn zoom_stops_at_a_few_frames_and_at_the_whole() {
        let near = ten_seconds().zoomed(1e9, 0.5);
        assert!(close(
            near.length,
            (FRAME * MIN_FRAMES).as_secs_f64()
        ));
        assert_eq!(ten_seconds().zoomed(0.1, 0.5), ten_seconds());
    }

    #[test]
    fn zooming_around_a_time_out_of_view_centres_it() {
        let view = ten_seconds().zoomed(10.0, 0.0);
        let zoomed = view.zoomed_around(1.0, 6.0);
        assert!(close(zoomed.time_at(0.5), 6.0));
    }

    #[test]
    fn the_view_stays_inside_the_animation() {
        let view = ten_seconds().zoomed(5.0, 0.5);
        assert_eq!(view.panned(-100.0).start, 0.0);
        assert!(close(view.panned(100.0).end(), 10.0));
        assert_eq!(view.fitted(), ten_seconds());
    }

    #[test]
    fn a_seek_to_either_edge_stays_on_the_page() {
        // The clock holds time as a `Duration`, which rounds to the
        // nanosecond; an edge must survive that round trip.
        let round_trip = |seconds: f64| {
            Duration::from_secs_f64(seconds).as_secs_f64()
        };
        for zoom in [3.0, 7.0, 11.0, 37.0] {
            for anchor in [0.13, 0.37, 0.61, 0.97] {
                let view =
                    Viewport::whole(Duration::from_secs_f64(13.7))
                        .zoomed(zoom, anchor);
                assert_eq!(
                    view.following(round_trip(view.end())),
                    view
                );
                assert_eq!(
                    view.following(round_trip(view.start)),
                    view
                );
            }
        }
    }

    #[test]
    fn playback_turns_pages() {
        let view = ten_seconds().zoomed(5.0, 0.0);
        assert_eq!(view.following(1.0), view);
        assert!(close(view.following(2.5).start, 2.5));
    }

    #[test]
    fn ruler_labels_get_room_whatever_the_width() {
        for width in [300.0, 860.0, 2000.0] {
            for length in [4.0 / 60.0, 0.5, 3.0, 10.0, 75.0, 900.0] {
                let (major, _) = ruler_step(length, width);
                assert!(
                    major / length * width >= LABEL_GAP,
                    "{length} s across {width} px"
                );
            }
        }
        // A narrow track labels less often than a wide one.
        assert!(
            ruler_step(10.0, 300.0).0 > ruler_step(10.0, 2000.0).0
        );
        // Before layout, about ten labels.
        assert!(close(ruler_step(4.0 / 60.0, 0.0).0, 1.0 / 60.0));
    }
}
