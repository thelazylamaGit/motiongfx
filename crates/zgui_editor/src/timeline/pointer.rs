//! What the pointer does on the track: scrub the playhead, drag the
//! navigator, scroll past the edges of a zoomed view, and pan or zoom
//! with the wheel.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use zgui::prelude::*;
use zgui::view::FrameHandle;
use zgui::vocab::Timestamp;

use super::draw::on_navigator;
use super::viewport::Viewport;
use crate::clock::Clock;

/// How much one CSS pixel of wheel zooms, as an exponent, so zooming in
/// and back out by the same amount returns to the same view.
const WHEEL_ZOOM: f64 = 0.004;

/// How fast the track scrolls while the playhead is dragged past its
/// edge, in CSS pixels per second: a base, plus this much per pixel the
/// pointer is past the edge, up to a limit. Further out scrolls faster,
/// as in most editors.
const EDGE_BASE: f64 = 150.0;
const EDGE_GAIN: f64 = 12.0;
const EDGE_MAX: f64 = 2400.0;

/// The longest gap between two edge-scroll frames that is scrolled in
/// full; a stall or a window drag shouldn't jump the view.
const EDGE_GAP: f64 = 0.05;

/// Where a pointer is over the track.
#[derive(Clone, Copy, Debug)]
pub(super) struct Spot {
    /// Across the track, as a fraction of its width; below 0 or above 1
    /// past either end.
    pub(super) fraction: f64,
    /// Down from the track's top, in CSS pixels.
    pub(super) y: f64,
    /// The track's size, in CSS pixels.
    pub(super) width: f64,
    pub(super) height: f64,
}

impl Spot {
    /// Where a pointer at (`x`, `y`) window CSS pixels is over `track`,
    /// once the track has been laid out.
    pub(super) fn locate(
        track: NodeRef,
        x: f32,
        y: f32,
    ) -> Option<Self> {
        // Boxes are in device pixels and pointers in CSS pixels; the
        // window's scale relates them.
        let bounds = track.window_bounds()?;
        let scale = track.scale();
        if bounds.size.width.0 <= 0.0 || scale <= 0.0 {
            return None;
        }
        Some(Self {
            fraction: f64::from(
                (x * scale - bounds.origin.x.0) / bounds.size.width.0,
            ),
            y: f64::from((y * scale - bounds.origin.y.0) / scale),
            width: f64::from(bounds.size.width.0 / scale),
            height: f64::from(bounds.size.height.0 / scale),
        })
    }
}

/// What a press on the track started.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Idle,
    /// Moving the playhead.
    Scrub,
    /// Moving the navigator's thumb, held `grab` seconds after the view's
    /// start.
    Navigate {
        grab: f64,
    },
}

/// The track's pointer handling. Cloned into each listener; the clones
/// share one drag.
#[derive(Clone)]
pub(super) struct TrackPointer(Rc<Inner>);

struct Inner {
    clock: Clock,
    viewport: RwSignal<Viewport, LocalStorage>,
    drag: Cell<Drag>,
    edge: EdgeScroll,
}

/// Scrolling past an edge of a zoomed view. It is paced by the frame
/// clock rather than by how often the pointer reports, so it is smooth
/// and can't run away.
struct EdgeScroll {
    /// How far past the edge the pointer is, in CSS pixels: negative
    /// before the start, zero inside.
    overshoot: Cell<f64>,
    /// The track's width in CSS pixels.
    width: Cell<f64>,
    /// When the last scroll frame ran.
    last: Cell<Option<Timestamp>>,
    /// The next scroll frame, while scrolling.
    frame: RefCell<Option<FrameHandle>>,
    timers: Timers,
}

impl TrackPointer {
    /// Pointer handling that moves `clock` and `viewport`.
    ///
    /// # Panics
    ///
    /// Outside a window, which paces the edge scroll.
    pub(super) fn new(
        clock: Clock,
        viewport: RwSignal<Viewport, LocalStorage>,
    ) -> Self {
        Self(Rc::new(Inner {
            clock,
            viewport,
            drag: Cell::new(Drag::Idle),
            edge: EdgeScroll {
                overshoot: Cell::new(0.0),
                width: Cell::new(0.0),
                last: Cell::new(None),
                frame: RefCell::new(None),
                timers: Timers::current()
                    .expect("the timeline is built in a window"),
            },
        }))
    }

    /// Whether a drag is under way; while it is, the drag decides what is
    /// in view.
    pub(super) fn is_dragging(&self) -> bool {
        self.0.drag.get() != Drag::Idle
    }

    /// A press at `spot`: on the navigator while zoomed, start dragging
    /// it; anywhere else, start scrubbing.
    pub(super) fn press(&self, spot: Spot) {
        let view = self.0.viewport.get_untracked();
        if view.is_zoomed() && on_navigator(spot.y, spot.height) {
            // Grabbing the thumb keeps it under the pointer; pressing
            // beside it centres it there first.
            let at = spot.fraction * view.duration();
            let grab = if (view.start()..=view.end()).contains(&at) {
                at - view.start()
            } else {
                view.length() / 2.0
            };
            self.0.drag.set(Drag::Navigate { grab });
            self.navigate_to(spot.fraction, grab);
        } else {
            self.0.drag.set(Drag::Scrub);
            self.scrub_to(spot);
        }
    }

    /// The pointer moved to `spot`, continuing whatever the press started.
    pub(super) fn drag_to(&self, spot: Spot) {
        match self.0.drag.get() {
            Drag::Idle => {}
            Drag::Scrub => self.scrub_to(spot),
            Drag::Navigate { grab } => {
                self.navigate_to(spot.fraction, grab)
            }
        }
    }

    /// The press ended or was cancelled.
    pub(super) fn release(&self) {
        self.0.drag.set(Drag::Idle);
        self.stop_edge_scroll();
    }

    /// The wheel turned by (`dx`, `dy`) CSS pixels over `spot`: zoom
    /// around the pointer with `zoom` (Ctrl, Cmd or Alt held, as in most
    /// editors), otherwise scroll through time whichever way it turned.
    pub(super) fn wheel(
        &self,
        spot: Spot,
        dx: f64,
        dy: f64,
        zoom: bool,
    ) {
        let view = self.0.viewport.get_untracked();
        let next = if zoom {
            let factor = (-dy * WHEEL_ZOOM).exp();
            view.zoomed(factor, spot.fraction.clamp(0.0, 1.0))
        } else {
            view.panned((dx + dy) / spot.width * view.length())
        };
        self.show(next);
    }

    /// Inside the track the playhead follows the pointer. Past either end
    /// of a zoomed view it waits on the edge while the track scrolls
    /// under it.
    fn scrub_to(&self, spot: Spot) {
        let view = self.0.viewport.get_untracked();
        let overshoot = if !view.is_zoomed() {
            0.0
        } else if spot.fraction < 0.0 {
            spot.fraction * spot.width
        } else {
            (spot.fraction - 1.0).max(0.0) * spot.width
        };
        if overshoot == 0.0 {
            let time = view.time_at(spot.fraction.clamp(0.0, 1.0));
            self.0.clock.seek(Duration::from_secs_f64(time));
        }
        self.reach(overshoot, spot.width);
    }

    /// Move the view so the navigator's thumb, held `grab` seconds after
    /// its start, is under `fraction` of the bar. The bar spans the whole
    /// animation.
    fn navigate_to(&self, fraction: f64, grab: f64) {
        let view = self.0.viewport.get_untracked();
        self.show(
            view.starting_at(fraction * view.duration() - grab),
        );
    }

    /// The pointer is `overshoot` pixels past an edge of a track `width`
    /// wide: start scrolling if it has just gone past, stop if it is back.
    fn reach(&self, overshoot: f64, width: f64) {
        let edge = &self.0.edge;
        edge.width.set(width);
        let was = edge.overshoot.replace(overshoot);
        if overshoot != 0.0 && was == 0.0 {
            edge.last.set(None);
            self.next_edge_frame();
        } else if overshoot == 0.0 {
            self.stop_edge_scroll();
        }
    }

    fn stop_edge_scroll(&self) {
        self.0.edge.overshoot.set(0.0);
        self.0.edge.frame.borrow_mut().take();
    }

    fn next_edge_frame(&self) {
        let pointer = self.clone();
        let next = self.0.edge.timers.request_frame(move |at| {
            if pointer.edge_frame(at) {
                pointer.next_edge_frame();
            }
        });
        *self.0.edge.frame.borrow_mut() = Some(next);
    }

    /// One frame of edge scrolling at `at`: scroll for the time since the
    /// last frame and keep the playhead on the edge. Whether to go on.
    fn edge_frame(&self, at: Timestamp) -> bool {
        let edge = &self.0.edge;
        let (overshoot, width) =
            (edge.overshoot.get(), edge.width.get());
        if overshoot == 0.0 || width <= 0.0 {
            return false;
        }
        let elapsed =
            edge.last.replace(Some(at)).map_or(0.0, |last| {
                at.saturating_since(last).as_secs_f64().min(EDGE_GAP)
            });
        let speed =
            (EDGE_BASE + overshoot.abs() * EDGE_GAIN).min(EDGE_MAX);
        let view = self.0.viewport.get_untracked();
        let pixels = overshoot.signum() * speed * elapsed;
        let next = view.panned(pixels / width * view.length());
        self.show(next);
        let held = if overshoot > 0.0 {
            next.end()
        } else {
            next.start()
        };
        self.0.clock.seek(Duration::from_secs_f64(held));
        true
    }

    /// Show `view`, if it differs from what is shown.
    fn show(&self, view: Viewport) {
        if self.0.viewport.get_untracked() != view {
            self.0.viewport.set(view);
        }
    }
}
