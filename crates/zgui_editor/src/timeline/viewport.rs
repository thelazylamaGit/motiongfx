//! The stretch of the animation the track shows, and how zooming,
//! panning and playback move it.

use std::time::Duration;

use super::timecode::FRAME;

/// How much `=` and `-` zoom per press.
pub(crate) const ZOOM_STEP: f64 = 1.5;

/// The fewest frames the track can be zoomed to show.
const MIN_FRAMES: u32 = 4;

/// The stretch of the animation the track shows, in seconds.
///
/// It always lies inside the animation: every way of making a new one
/// clamps it there.
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

    /// The first time in view.
    pub(crate) fn start(self) -> f64 {
        self.start
    }

    /// How much time is in view.
    pub(crate) fn length(self) -> f64 {
        self.length
    }

    /// The last time in view.
    pub(crate) fn end(self) -> f64 {
        self.start + self.length
    }

    /// The whole animation's length.
    pub(crate) fn duration(self) -> f64 {
        self.duration
    }

    /// Whether only part of the animation is in view.
    pub(crate) fn is_zoomed(self) -> bool {
        self.length < self.duration
    }

    /// The time at `fraction` of the track's width.
    pub(crate) fn time_at(self, fraction: f64) -> f64 {
        self.start + fraction * self.length
    }

    /// Where `seconds` falls, as a fraction of the track's width.
    pub(crate) fn fraction_of(self, seconds: f64) -> f64 {
        (seconds - self.start) / self.length
    }

    /// The same length of view, starting as near `start` as the
    /// animation allows.
    pub(crate) fn starting_at(self, start: f64) -> Self {
        Self {
            start: start.clamp(0.0, self.duration - self.length),
            ..self
        }
    }

    /// Moved `seconds` later (earlier, below zero).
    pub(crate) fn panned(self, seconds: f64) -> Self {
        self.starting_at(self.start + seconds)
    }

    /// Zoomed in by `factor` (out, below one), keeping the time at
    /// `anchor` (a fraction of the width) where it is. Stops at a few
    /// frames across and at the whole animation.
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

    /// Turned a page, if need be, so `seconds` is in view: how an editor
    /// keeps up with the playhead.
    pub(crate) fn following(self, seconds: f64) -> Self {
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
}
