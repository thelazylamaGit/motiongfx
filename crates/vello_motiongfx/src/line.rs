//! A styled line with independently animatable endpoints; no path allocation.

use crate::{
    Color, DrawObject, Graphics, Point, Position, Scene, SceneObject,
};
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
use std::sync::Arc;
use vello::kurbo::{Affine, Line as LineShape, PathEl, Stroke, Vec2};
use vello::peniko::Fill;

#[derive(Clone)]
pub struct Line {
    pub start: Point,
    pub end: Point,
    pub stroke: Color,
    pub stroke_width: f64,
    pub opacity: f64,
    /// Visible fraction from start towards end, clamped to 0..1 in rendering.
    pub reveal: f64,
    pub line_dash: Arc<[f64]>,
    pub dash_offset: f64,
    pub start_offset: f64,
    pub end_offset: f64,
    pub start_arrow: bool,
    pub end_arrow: bool,
    pub arrow_size: f64,
}
impl Line {
    pub fn new(
        start: impl Into<crate::Input<Point>>,
        end: impl Into<crate::Input<Point>>,
    ) -> crate::Object<Self> {
        crate::Object::new(Self {
            start: Point::ZERO,
            end: Point::ZERO,
            stroke: Color::WHITE,
            stroke_width: 1.0,
            opacity: 1.0,
            reveal: 1.0,
            line_dash: Arc::from([]),
            dash_offset: 0.0,
            start_offset: 0.0,
            end_offset: 0.0,
            start_arrow: false,
            end_arrow: false,
            arrow_size: 24.0,
        })
        .start(start)
        .end(end)
    }
    pub(crate) fn render(
        &self,
        scene: &mut vello::Scene,
        screen: Affine,
    ) {
        let opacity = self.opacity.clamp(0.0, 1.0) as f32;
        if opacity == 0.0
            || self.stroke_width <= 0.0
            || self.reveal <= 0.0
        {
            return;
        }
        let mut start = self.start;
        let mut end = self.start
            + (self.end - self.start) * self.reveal.clamp(0.0, 1.0);
        let mut arrows = None;
        // Undecorated lines keep the simple endpoint interpolation path.
        if self.start_offset > 0.0
            || self.end_offset > 0.0
            || self.start_arrow
            || self.end_arrow
        {
            let delta = self.end - self.start;
            let length = delta.hypot();
            if !length.is_finite() || length <= 0.0 {
                return;
            }
            let direction = delta / length;
            let distance = (length
                - self.start_offset.max(0.0)
                - self.end_offset.max(0.0))
            .max(0.0)
                * self.reveal.clamp(0.0, 1.0);
            if distance <= 0.0 {
                return;
            }
            let tip_start = self.start
                + direction * self.start_offset.max(0.0).min(length);
            let tip_end = tip_start + direction * distance;
            let size = self.arrow_size.max(0.0).min(distance / 2.0);
            start = tip_start
                + direction
                    * if self.start_arrow { size / 2.0 } else { 0.0 };
            end = tip_end
                - direction
                    * if self.end_arrow { size / 2.0 } else { 0.0 };
            arrows = Some((tip_start, tip_end, direction, size));
        }
        let color = self
            .stroke
            .with_alpha(self.stroke.components[3] * opacity);
        let mut stroke = Stroke::new(self.stroke_width);
        if !self.line_dash.is_empty() {
            assert!(
                self.dash_offset.is_finite()
                    && self
                        .line_dash
                        .iter()
                        .all(|d| d.is_finite() && *d >= 0.0)
                    && self.line_dash.iter().sum::<f64>() > 0.0,
                "line dash lengths must be finite, nonnegative and have a positive period"
            );
            stroke = stroke
                .with_dashes(self.dash_offset, self.line_dash.iter());
        }
        let shape =
            LineShape::new((start.x, -start.y), (end.x, -end.y));
        scene.stroke(&stroke, screen, color, None, &shape);
        if let Some((start, end, direction, size)) = arrows
            && size > 0.0
        {
            if self.start_arrow {
                draw_arrow(
                    scene, screen, color, start, direction, size,
                );
            }
            if self.end_arrow {
                draw_arrow(
                    scene, screen, color, end, -direction, size,
                );
            }
        }
    }
}

fn draw_arrow(
    scene: &mut vello::Scene,
    screen: Affine,
    color: Color,
    tip: Point,
    inward: Vec2,
    size: f64,
) {
    let base = tip + inward * size;
    let normal = Vec2::new(-inward.y, inward.x) * size;
    let canvas = |p: Point| Point::new(p.x, -p.y);
    // Fixed-size stack geometry; arrowheads do not allocate a path buffer.
    let triangle = [
        PathEl::MoveTo(canvas(tip)),
        PathEl::LineTo(canvas(base + normal)),
        PathEl::LineTo(canvas(base - normal)),
        PathEl::ClosePath,
    ];
    scene.fill(Fill::NonZero, screen, color, None, &triangle);
}

crate::object::properties!(Line;
    start: Point, end: Point, opacity: f64, stroke_width: f64, reveal: f64,
    line_dash: Arc<[f64]>, dash_offset: f64, start_offset: f64, end_offset: f64,
    start_arrow: bool, end_arrow: bool, arrow_size: f64,
);
crate::object::stroke!(Line);

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct LineId(pub(crate) usize);
#[derive(Clone, Copy)]
pub struct LineHandle {
    pub(crate) id: LineId,
    pub start: Position<LineId, Line>,
    pub end: Position<LineId, Line>,
    pub stroke: Property<LineId, Line, Color>,
    pub stroke_width: Property<LineId, Line, f64>,
    pub opacity: Property<LineId, Line, f64>,
    pub reveal: Property<LineId, Line, f64>,
    pub line_dash: Property<LineId, Line, Arc<[f64]>>,
    pub dash_offset: Property<LineId, Line, f64>,
    pub start_offset: Property<LineId, Line, f64>,
    pub end_offset: Property<LineId, Line, f64>,
    pub start_arrow: Property<LineId, Line, bool>,
    pub end_arrow: Property<LineId, Line, bool>,
    pub arrow_size: Property<LineId, Line, f64>,
}
impl SceneObject for Line {
    type Handle = LineHandle;
    fn insert_into(self, scene: &mut Scene) -> LineHandle {
        let id = LineId(scene.graphics.lines.len());
        scene.graphics.lines.push(self);
        scene.graphics.order.push(DrawObject::Line(id));
        LineHandle {
            id,
            start: Position::new(
                id,
                path!(Line.start),
                path!(Line.start.x),
                path!(Line.start.y),
            ),
            end: Position::new(
                id,
                path!(Line.end),
                path!(Line.end.x),
                path!(Line.end.y),
            ),
            stroke: Property::new(id, path!(Line.stroke)),
            stroke_width: Property::new(id, path!(Line.stroke_width)),
            opacity: Property::new(id, path!(Line.opacity)),
            reveal: Property::new(id, path!(Line.reveal)),
            line_dash: Property::discrete(id, path!(Line.line_dash)),
            dash_offset: Property::new(id, path!(Line.dash_offset)),
            start_offset: Property::new(id, path!(Line.start_offset)),
            end_offset: Property::new(id, path!(Line.end_offset)),
            start_arrow: Property::discrete(
                id,
                path!(Line.start_arrow),
            ),
            end_arrow: Property::discrete(id, path!(Line.end_arrow)),
            arrow_size: Property::new(id, path!(Line.arrow_size)),
        }
    }
}
impl SubjectSource<LineId, Line> for Graphics {
    fn get_source(&self, id: LineId) -> Option<&Line> {
        self.lines.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: LineId,
        f: impl FnOnce(&mut Line) -> R,
    ) -> Option<R> {
        self.lines.get_mut(id.0).map(f)
    }
}

impl crate::Animation {
    pub fn line(&self, handle: LineHandle) -> &Line {
        &self.graphics.lines[handle.id.0]
    }
}
