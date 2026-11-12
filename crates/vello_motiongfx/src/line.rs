//! A styled line with independently animatable endpoints; no path allocation.

use crate::{
    Color, DrawObject, Graphics, Point, Position, Scene, SceneObject,
};
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
use vello::kurbo::{Affine, Line as LineShape, Stroke};

#[derive(Clone)]
pub struct Line {
    pub start: Point,
    pub end: Point,
    pub stroke: Color,
    pub stroke_width: f64,
    pub opacity: f64,
    /// Visible fraction from start towards end, clamped to 0..1 in rendering.
    pub reveal: f64,
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
        let end = self.start
            + (self.end - self.start) * self.reveal.clamp(0.0, 1.0);
        let shape = LineShape::new(
            (self.start.x, -self.start.y),
            (end.x, -end.y),
        );
        scene.stroke(
            &Stroke::new(self.stroke_width),
            screen,
            self.stroke
                .with_alpha(self.stroke.components[3] * opacity),
            None,
            &shape,
        );
    }
}

crate::object::properties!(Line; start: Point, end: Point, opacity: f64, stroke_width: f64, reveal: f64);
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
