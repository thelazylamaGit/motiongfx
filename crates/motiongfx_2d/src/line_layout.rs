//! Shared endpoint placement, reveal and shaft trimming for any 2D renderer.
use crate::{
    EndpointTip, Geometry, Graphics, Line, Point, TriangleId,
};

/// Visible shaft and endpoint placements, in y-up canvas coordinates.
/// This small value contains no allocated geometry or renderer commands.
pub struct LineLayout {
    pub shaft: Option<(Point, Point)>,
    pub tips: [Option<TipPlacement>; 2],
}

/// An attached shape's frame; the shape's own transform is local to this frame.
pub struct TipPlacement {
    pub tip: EndpointTip,
    pub position: Point,
    pub rotation: f64,
    pub scale: f64,
}

impl Line {
    /// Resolve portable line geometry. The adapter supplies prepared triangle
    /// geometry so filled-tip trimming uses the same curved boundary as drawing.
    /// Ordinary lines take a fast path without length/orientation calculations.
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
