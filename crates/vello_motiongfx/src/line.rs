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
}
impl Line {
    pub fn new(
        start: impl Into<Point>,
        end: impl Into<Point>,
    ) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            stroke: Color::WHITE,
            stroke_width: 1.0,
            opacity: 1.0,
        }
    }
    pub fn stroke(mut self, color: Color, width: f64) -> Self {
        self.stroke = color;
        self.stroke_width = width;
        self
    }
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }
    pub(crate) fn render(
        &self,
        scene: &mut vello::Scene,
        screen: Affine,
    ) {
        let opacity = self.opacity.clamp(0.0, 1.0) as f32;
        if opacity == 0.0 || self.stroke_width <= 0.0 {
            return;
        }
        let shape = LineShape::new(
            (self.start.x, -self.start.y),
            (self.end.x, -self.end.y),
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
