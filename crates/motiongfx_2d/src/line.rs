//! [`Line`]: straight lines, optionally dashed, trimmed or with tips.

use crate::tips::EndpointTip;
use crate::{Color, DrawObject, Point, SceneObject};
use std::sync::Arc;

/// A straight line between two points. Create one with [`Line::new`];
/// add arrowheads or dots with `start_tip`/`end_tip`.
#[derive(Clone)]
pub struct Line {
    /// Start point, in canvas units (y up).
    pub start: Point,
    /// End point, in canvas units (y up).
    pub end: Point,
    /// Line colour.
    pub stroke: Color,
    /// Line width in canvas units.
    pub stroke_width: f64,
    /// How open stroke ends and dash ends are drawn.
    pub line_cap: crate::Cap,
    /// How stroke corners are drawn.
    pub line_join: crate::Join,
    /// 0 (invisible) to 1 (opaque).
    pub opacity: f64,
    /// How much of the line is drawn, from 0 (none) to 1 (all), starting
    /// at `start`.
    pub reveal: f64,
    /// Dash pattern: alternating dash and gap lengths. Empty for a solid line.
    pub line_dash: Arc<[f64]>,
    /// How far into the dash pattern the line starts.
    pub dash_offset: f64,
    /// Distance trimmed from the start, in canvas units.
    pub start_offset: f64,
    /// Distance trimmed from the end, in canvas units.
    pub end_offset: f64,
    pub(crate) start_tip: Option<EndpointTip>,
    pub(crate) end_tip: Option<EndpointTip>,
}
impl Line {
    /// The shape at the start, if any.
    pub fn start_tip(&self) -> Option<EndpointTip> {
        self.start_tip
    }
    /// The shape at the end, if any.
    pub fn end_tip(&self) -> Option<EndpointTip> {
        self.end_tip
    }
    /// A white line from `start` to `end` (fixed points, or sources to
    /// follow).
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
crate::object::stroke!(Line);
crate::object::stroke_options!(Line);

/// Identifies one [`Line`] in its scene.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct LineId(pub(crate) usize);
impl LineId {
    /// Its position in the scene's storage for this object type.
    pub fn index(self) -> usize {
        self.0
    }
}
/// A [`Line`] in a scene, returned by `scene.add`. Its
/// methods return fields to animate or for other objects to follow.
#[derive(Clone, Copy, Debug)]
pub struct LineHandle {
    pub(crate) id: LineId,
}
impl LineHandle {
    /// The object's ID.
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
crate::capability::stored_in!(Line, LineId, lines);

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
crate::properties::object!(Line, LineHandle, LineId;
also impl<S, E> crate::LineTipHandle<S, E> => line;
set start: Point => new(Line.start);
set end: Point => new(Line.end);
set opacity: f64 => new(Line.opacity);
get stroke: Color => new(Line.stroke);
set stroke_width: f64 => new(Line.stroke_width);
get line_cap: crate::Cap => discrete(Line.line_cap);
get line_join: crate::Join => discrete(Line.line_join);
set reveal: f64 => new(Line.reveal);
set line_dash: Arc<[f64]> => discrete(Line.line_dash);
set dash_offset: f64 => new(Line.dash_offset);
set start_offset: f64 => new(Line.start_offset);
set end_offset: f64 => new(Line.end_offset);
get start_x: f64 => new(Line.start.x);
get start_y: f64 => new(Line.start.y);
get end_x: f64 => new(Line.end.x);
get end_y: f64 => new(Line.end.y);
);
crate::capability::handle_traits!(LineHandle => Line, LineId; Strokable, Fadable);
crate::capability::handle_traits!(impl [S: Copy, E: Copy] crate::LineTipHandle<S, E> => Line, LineId,
    id: line.id; Strokable, Fadable);
