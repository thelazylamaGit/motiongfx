//! A styled line with independently animatable endpoints; no path allocation.

use crate::tips::EndpointTip;
use crate::{Color, DrawObject, Point, Position, SceneObject};
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
use std::sync::Arc;

#[derive(Clone)]
pub struct Line {
    pub start: Point,
    pub end: Point,
    pub stroke: Color,
    pub stroke_width: f64,
    pub line_cap: crate::Cap,
    pub line_join: crate::Join,
    pub opacity: f64,
    /// Visible fraction from start towards end, clamped to 0..1 in rendering.
    pub reveal: f64,
    pub line_dash: Arc<[f64]>,
    pub dash_offset: f64,
    pub start_offset: f64,
    pub end_offset: f64,
    pub(crate) start_tip: Option<EndpointTip>,
    pub(crate) end_tip: Option<EndpointTip>,
}
impl Line {
    pub fn start_tip(&self) -> Option<EndpointTip> {
        self.start_tip
    }
    pub fn end_tip(&self) -> Option<EndpointTip> {
        self.end_tip
    }
    pub fn new<M: crate::Model2d>(
        start: impl Into<crate::Input<Point, M>>,
        end: impl Into<crate::Input<Point, M>>,
    ) -> crate::Object<Self, M> {
        crate::Object::new(Self {
            start: Point::ZERO,
            end: Point::ZERO,
            stroke: Color::WHITE,
            stroke_width: 1.0,
            line_cap: crate::Cap::Round,
            line_join: crate::Join::Round,
            opacity: 1.0,
            reveal: 1.0,
            line_dash: Arc::from([]),
            dash_offset: 0.0,
            start_offset: 0.0,
            end_offset: 0.0,
            start_tip: None,
            end_tip: None,
        })
        .start(start)
        .end(end)
    }
}
crate::object::properties!(Line;
    start: Point, end: Point, opacity: f64, stroke_width: f64, reveal: f64,
    line_dash: Arc<[f64]>, dash_offset: f64, start_offset: f64, end_offset: f64,

);
crate::object::stroke!(Line);
crate::object::stroke_options!(Line);

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct LineId(pub(crate) usize);
impl LineId {
    pub fn index(self) -> usize {
        self.0
    }
}
#[derive(Clone, Copy)]
pub struct LineHandle {
    pub(crate) id: LineId,
    pub start: Position<LineId, Line>,
    pub end: Position<LineId, Line>,
    pub stroke: Property<LineId, Line, Color>,
    pub stroke_width: Property<LineId, Line, f64>,
    pub line_cap: Property<LineId, Line, crate::Cap>,
    pub line_join: Property<LineId, Line, crate::Join>,
    pub opacity: Property<LineId, Line, f64>,
    pub reveal: Property<LineId, Line, f64>,
    pub line_dash: Property<LineId, Line, Arc<[f64]>>,
    pub dash_offset: Property<LineId, Line, f64>,
    pub start_offset: Property<LineId, Line, f64>,
    pub end_offset: Property<LineId, Line, f64>,
}
impl LineHandle {
    pub fn id(self) -> LineId {
        self.id
    }
}
impl<M: crate::Model2d> SceneObject<M> for Line {
    type Handle = LineHandle;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> LineHandle {
        let id = LineId(scene.model_mut().graphics_mut().lines.len());
        scene.model_mut().graphics_mut().lines.push(self);
        scene
            .model_mut()
            .graphics_mut()
            .order
            .push(DrawObject::Line(id));
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
            line_cap: Property::discrete(id, path!(Line.line_cap)),
            line_join: Property::discrete(id, path!(Line.line_join)),
            stroke_width: Property::new(id, path!(Line.stroke_width)),
            opacity: Property::new(id, path!(Line.opacity)),
            reveal: Property::new(id, path!(Line.reveal)),
            line_dash: Property::discrete(id, path!(Line.line_dash)),
            dash_offset: Property::new(id, path!(Line.dash_offset)),
            start_offset: Property::new(id, path!(Line.start_offset)),
            end_offset: Property::new(id, path!(Line.end_offset)),
        }
    }
}
impl<M: crate::Model2d> SubjectSource<LineId, Line>
    for motiongfx_authoring::World<M>
{
    fn get_source(&self, id: LineId) -> Option<&Line> {
        self.model().graphics().lines.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: LineId,
        f: impl FnOnce(&mut Line) -> R,
    ) -> Option<R> {
        self.model_mut().graphics_mut().lines.get_mut(id.0).map(f)
    }
}
