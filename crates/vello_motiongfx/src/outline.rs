//! [`Outline`]: the stroke each object draws its outline with.

use motiongfx_2d::{
    Cap, Chevron, Circle, Color, Join, Line, Path, Rectangle,
    Triangle,
};
use vello::kurbo::Stroke;

/// How to stroke an object's outline: the stroke style, and its colour with
/// the object's opacity applied. `None` if the outline wouldn't show, so
/// invisible outlines never become draw commands.
pub(crate) trait Outline {
    fn outline(&self, opacity: f32) -> Option<(Stroke, Color)>;
}

/// `color` with `opacity` applied, if an outline `width` wide would show.
fn visible(color: Color, width: f64, opacity: f32) -> Option<Color> {
    let alpha = color.components[3] * opacity;
    (width > 0.0 && alpha > 0.0).then(|| color.with_alpha(alpha))
}

/// A visible outline with ends and corners.
fn styled(
    color: Color,
    width: f64,
    (cap, join): (Cap, Join),
    opacity: f32,
) -> Option<(Stroke, Color)> {
    visible(color, width, opacity).map(|color| {
        (Stroke::new(width).with_caps(cap).with_join(join), color)
    })
}

impl Outline for Rectangle {
    fn outline(&self, opacity: f32) -> Option<(Stroke, Color)> {
        let style = (self.line_cap, self.line_join);
        styled(self.stroke, self.stroke_width, style, opacity)
    }
}

impl Outline for Path {
    fn outline(&self, opacity: f32) -> Option<(Stroke, Color)> {
        let style = (self.line_cap, self.line_join);
        styled(self.stroke, self.stroke_width, style, opacity)
    }
}

impl Outline for Triangle {
    fn outline(&self, opacity: f32) -> Option<(Stroke, Color)> {
        let style = (self.line_cap, self.line_join);
        styled(self.stroke, self.stroke_width, style, opacity)
    }
}

impl Outline for Chevron {
    fn outline(&self, opacity: f32) -> Option<(Stroke, Color)> {
        let style = (self.line_cap, self.line_join);
        styled(self.stroke, self.stroke_width, style, opacity)
    }
}

// A closed curve has no ends or corners, so a circle has no cap or join.
impl Outline for Circle {
    fn outline(&self, opacity: f32) -> Option<(Stroke, Color)> {
        visible(self.stroke, self.stroke_width, opacity)
            .map(|color| (Stroke::new(self.stroke_width), color))
    }
}

impl Outline for Line {
    fn outline(&self, opacity: f32) -> Option<(Stroke, Color)> {
        let style = (self.line_cap, self.line_join);
        let (stroke, color) =
            styled(self.stroke, self.stroke_width, style, opacity)?;
        if self.line_dash.is_empty() {
            return Some((stroke, color));
        }
        assert!(
            self.dash_offset.is_finite()
                && self
                    .line_dash
                    .iter()
                    .all(|d| d.is_finite() && *d >= 0.0)
                && self.line_dash.iter().sum::<f64>() > 0.0,
            "line dash lengths must be finite, nonnegative and have a positive period"
        );
        let dashed = stroke
            .with_dashes(self.dash_offset, self.line_dash.iter());
        Some((dashed, color))
    }
}
