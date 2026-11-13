//! A styled line with independently animatable endpoints; no path allocation.

use crate::tips::EndpointTip;
use crate::{Color, DrawObject, Point, SceneObject};
use motiongfx::prelude::SubjectSource;
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
        LineHandle { id }
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

// Fixed field identity is generated locally; no accessor metadata is stored in
// each handle. See properties.rs and motiongfx_authoring::Target.
crate::properties::fields!(fields, Line, LineId;
stroke: Color => new(Line.stroke);
stroke_width: f64 => new(Line.stroke_width);
line_cap: crate::Cap => discrete(Line.line_cap);
line_join: crate::Join => discrete(Line.line_join);
opacity: f64 => new(Line.opacity);
reveal: f64 => new(Line.reveal);
line_dash: Arc<[f64]> => discrete(Line.line_dash);
dash_offset: f64 => new(Line.dash_offset);
start_offset: f64 => new(Line.start_offset);
end_offset: f64 => new(Line.end_offset);
start: Point => new(Line.start);
start_x: f64 => new(Line.start.x);
start_y: f64 => new(Line.start.y);
end: Point => new(Line.end);
end_x: f64 => new(Line.end.x);
end_y: f64 => new(Line.end.y);
);
#[allow(non_camel_case_types)]
pub type line_start = crate::Position<
    LineId,
    Line,
    fields::start::Target,
    fields::start_x::Target,
    fields::start_y::Target,
>;
pub fn line_start_axes(id: LineId) -> line_start {
    crate::Position::from_targets(
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
    )
}
#[allow(non_camel_case_types)]
pub type line_end = crate::Position<
    LineId,
    Line,
    fields::end::Target,
    fields::end_x::Target,
    fields::end_y::Target,
>;
pub fn line_end_axes(id: LineId) -> line_end {
    crate::Position::from_targets(
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
    )
}

crate::properties::handle_methods!(LineHandle,Line,LineId;
stroke: Color;
stroke_width: f64;
line_cap: crate::Cap;
line_join: crate::Join;
opacity: f64;
reveal: f64;
line_dash: Arc<[f64]>;
dash_offset: f64;
start_offset: f64;
end_offset: f64;
start: Point;
start_x: f64;
start_y: f64;
end: Point;
end_x: f64;
end_y: f64;
);
