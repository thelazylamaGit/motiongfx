//! What the track's two canvases draw: the marks that move only with the
//! view (ruler ticks, frame stripes, the navigator), and the playhead with
//! the played range.
//!
//! Every shape is filled with its canvas's CSS text colour at some
//! opacity ([`Brush::Inherited`]), so colours live in the style sheet and
//! follow the colour scheme. Everything here is in CSS pixels from the
//! track's top-left corner.

use vello_winit_motiongfx::PlaybackState;
use zgui::canvas::{Brush, ShapeBuilder};
use zgui::elements::kurbo::{BezPath, Rect, RoundedRect, Shape as _};
use zgui::prelude::*;

use super::ruler::ruler_step;
use super::timecode::FRAME;
use super::viewport::Viewport;

/// The ruler's height; ticks stand on its bottom edge. The style sheet's
/// `.ruler` uses the same number.
pub(super) const RULER: f64 = 26.0;

/// Where the lane starts below the track's top, and ends above its
/// bottom; the style sheet's `.lane` uses the same numbers.
pub(super) const LANE_TOP: f64 = 30.0;
pub(super) const LANE_BOTTOM: f64 = 12.0;

/// Where the playhead's knob sits below the track's top.
const HEAD_TOP: f64 = 12.0;

/// The playhead's knob, and how far it may overhang the track's ends so a
/// playhead at the very start or end isn't cut in half.
const KNOB: f64 = 12.0;

/// The navigator bar under the lane, shown while zoomed: its top above
/// the track's bottom, and its height. Presses below the lane land on it.
const NAV_RAISE: f64 = 8.0;
const NAV_HEIGHT: f64 = 4.0;

/// The narrowest the navigator's thumb gets, so it can still be grabbed.
const THUMB_MIN: f64 = 16.0;

/// Frame stripes appear on the lane once a frame is this many CSS pixels
/// wide.
const STRIPE_MIN: f64 = 8.0;

/// How strongly each mark shows its canvas's colour.
const MINOR_TICK: f32 = 0.4;
const MAJOR_TICK: f32 = 0.8;
const STRIPE: f32 = 0.06;
const NAV_BAR: f32 = 0.15;
const NAV_THUMB: f32 = 0.55;
const PLAYED: f32 = 0.16;
const HEAD: f32 = 1.0;

/// The marks canvas for `view`, `width` by `height`.
pub(super) fn marks(
    cx: &mut DrawCx<'_>,
    view: Viewport,
    width: f64,
    height: f64,
) {
    ticks(cx, view, width);
    stripes(cx, view, width, height);
    if view.is_zoomed() {
        navigator(cx, view, width, height);
    }
}

/// The playhead canvas: the played part of the lane, the playhead, and
/// while zoomed, the playhead's place on the navigator.
pub(super) fn playhead(
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
            PLAYED,
        );
    }

    // The line and its knob are one path, so they move as one shape
    // rather than snapping to device pixels separately.
    let mut head = BezPath::new();
    let half = KNOB / 2.0;
    if (-half..=width + half).contains(&x) {
        let line = Rect::new(
            x - 1.0,
            HEAD_TOP,
            x + 1.0,
            height - LANE_BOTTOM,
        );
        let knob = RoundedRect::new(
            x - half,
            HEAD_TOP,
            x + half,
            HEAD_TOP + KNOB,
            3.0,
        );
        head.extend(line.path_elements(0.1));
        head.extend(knob.path_elements(0.1));
    }
    if view.is_zoomed() {
        let at = time / view.duration() * width;
        let top = height - NAV_RAISE;
        let mark = Rect::new(
            at - 1.0,
            top - 1.0,
            at + 1.0,
            top + NAV_HEIGHT + 1.0,
        );
        head.extend(mark.path_elements(0.1));
    }
    fill(cx, head, HEAD);
}

/// Whether a press `y` CSS pixels down a track `height` tall is on the
/// navigator rather than the lane.
pub(super) fn on_navigator(y: f64, height: f64) -> bool {
    y >= height - LANE_BOTTOM
}

/// The ruler's ticks along its bottom edge, each one device pixel wide so
/// they stay sharp at any display scale.
fn ticks(cx: &mut DrawCx<'_>, view: Viewport, width: f64) {
    let scale = f64::from(cx.scale).max(1.0);
    let (major, divisions) = ruler_step(view.length(), width);
    let minor = major / f64::from(divisions);
    let mut majors = BezPath::new();
    let mut minors = BezPath::new();
    let first = (view.start() / minor).floor() as i64;
    let last = (view.end() / minor).ceil() as i64;
    for index in first..=last {
        let at = index as f64 * minor;
        if at > view.duration() + 1e-9 {
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
        let tick =
            Rect::new(x, RULER - height, x + 1.0 / scale, RULER);
        path.extend(tick.path_elements(0.1));
    }
    fill(cx, minors, MINOR_TICK);
    fill(cx, majors, MAJOR_TICK);
}

/// Every other frame shaded on the lane, once frames are wide enough to
/// tell apart.
fn stripes(
    cx: &mut DrawCx<'_>,
    view: Viewport,
    width: f64,
    height: f64,
) {
    let frame = FRAME.as_secs_f64();
    if frame / view.length() * width < STRIPE_MIN {
        return;
    }
    let mut stripes = BezPath::new();
    let first = (view.start() / frame).floor() as i64;
    let last = (view.end() / frame).ceil() as i64;
    for index in
        (first..=last).filter(|index| index.rem_euclid(2) == 1)
    {
        let at = index as f64 * frame;
        let left = (view.fraction_of(at) * width).max(0.0);
        let right = (view.fraction_of(at + frame) * width).min(width);
        if right > left {
            let stripe = Rect::new(
                left,
                LANE_TOP,
                right,
                height - LANE_BOTTOM,
            );
            stripes.extend(stripe.path_elements(0.1));
        }
    }
    fill(cx, stripes, STRIPE);
}

/// The navigator: the whole animation as a bar, with the view as a thumb
/// to drag.
fn navigator(
    cx: &mut DrawCx<'_>,
    view: Viewport,
    width: f64,
    height: f64,
) {
    let top = height - NAV_RAISE;
    let bar =
        RoundedRect::new(0.0, top, width, top + NAV_HEIGHT, 2.0);
    fill(cx, bar.to_path(0.1), NAV_BAR);

    let mut left = view.start() / view.duration() * width;
    let mut right = view.end() / view.duration() * width;
    let shortfall = THUMB_MIN - (right - left);
    if shortfall > 0.0 {
        left = (left - shortfall / 2.0)
            .clamp(0.0, (width - THUMB_MIN).max(0.0));
        right = left + THUMB_MIN;
    }
    let thumb =
        RoundedRect::new(left, top, right, top + NAV_HEIGHT, 2.0);
    fill(cx, thumb.to_path(0.1), NAV_THUMB);
}

/// Fill `path`, if it has anything in it, with the canvas's text colour
/// at `alpha`.
fn fill(cx: &mut DrawCx<'_>, path: BezPath, alpha: f32) {
    if !path.is_empty() {
        let shape = ShapeBuilder::new(path)
            .fill(Brush::Inherited { alpha })
            .build();
        cx.scene.push(shape);
    }
}
