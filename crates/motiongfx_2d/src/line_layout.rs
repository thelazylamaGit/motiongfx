//! Where a line's visible shaft and tips go, for renderers to draw.
use crate::{
    EndpointTip, Geometry, Graphics, Line, Point, TriangleId,
};

/// Where to draw a line, after trimming, reveal and room for its tips, in
/// canvas units (y up).
pub struct LineLayout {
    /// The visible part of the line, if any is left.
    pub shaft: Option<(Point, Point)>,
    /// The start and end tips, if present and visible.
    pub tips: [Option<TipPlacement>; 2],
}

/// Where one tip goes. The tip shape's own position, rotation and scale are
/// applied on top of this.
pub struct TipPlacement {
    /// Which shape the tip is.
    pub tip: EndpointTip,
    /// The point the tip sits on.
    pub position: Point,
    /// The direction the tip faces (outwards along the line), in radians.
    pub rotation: f64,
    /// Shrinks tips that don't fit on a short line; 1 when they fit.
    pub scale: f64,
}

impl Line {
    /// Work out where to draw this line and its tips, or `None` if nothing
    /// is visible. `triangle_geometry` gives a triangle tip's outline, so the
    /// line stops where a rounded triangle's edge actually is.
    pub fn layout(
        &self,
        graphics: &Graphics,
        triangle_geometry: impl Fn(TriangleId) -> Geometry,
    ) -> Option<LineLayout> {
        if self.reveal <= 0.0 {
            return None;
        }
        if self.start_tip.is_none()
            && self.end_tip.is_none()
            && self.start_offset <= 0.0
            && self.end_offset <= 0.0
        {
            return Some(LineLayout {
                shaft: Some((
                    self.start,
                    self.start
                        + (self.end - self.start)
                            * self.reveal.clamp(0.0, 1.0),
                )),
                tips: [None, None],
            });
        }
        let delta = self.end - self.start;
        let length = delta.hypot();
        if !length.is_finite() || length <= 0.0 {
            return None;
        }
        let direction = delta / length;
        let distance = (length
            - self.start_offset.max(0.0)
            - self.end_offset.max(0.0))
        .max(0.0)
            * self.reveal.clamp(0.0, 1.0);
        if distance <= 0.0 {
            return None;
        }
        let tip_start = self.start
            + direction * self.start_offset.max(0.0).min(length);
        let tip_end = tip_start + direction * distance;
        let start_extent = self
            .start_tip
            .map_or(0.0, |t| t.extent(graphics, &triangle_geometry));
        let end_extent = self
            .end_tip
            .map_or(0.0, |t| t.extent(graphics, &triangle_geometry));
        let extent = start_extent + end_extent;
        // Fit both tips together; never reverse a short/revealing shaft.
        let fit = if extent > distance {
            distance / extent
        } else {
            1.0
        };
        let start = tip_start
            + direction
                * self
                    .start_tip
                    .map_or(0.0, |t| t.trim(start_extent))
                * fit;
        let end = tip_end
            - direction
                * self.end_tip.map_or(0.0, |t| t.trim(end_extent))
                * fit;
        let angle = direction.y.atan2(direction.x);
        Some(LineLayout {
            shaft: (start.distance_squared(end) > 1e-18)
                .then_some((start, end)),
            tips: [
                self.start_tip.map(|tip| TipPlacement {
                    tip,
                    position: tip_start,
                    rotation: angle + std::f64::consts::PI,
                    scale: fit,
                }),
                self.end_tip.map(|tip| TipPlacement {
                    tip,
                    position: tip_end,
                    rotation: angle,
                    scale: fit,
                }),
            ],
        })
    }
}
