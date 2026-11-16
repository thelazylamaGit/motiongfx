//! [`Path`]: stroked curves and polylines that can be drawn on gradually.
use crate::Handle;

use crate::{Color, DrawObject, Point, SceneObject};
pub use kurbo::BezPath;
use motiongfx::prelude::path;
use std::sync::Arc;

/// A stroked curve or polyline. Animate `end` from 0 to 1 to draw it on.
#[derive(Clone)]
pub struct Path {
    /// The path's shape, shared between copies.
    pub geometry: Arc<BezPath>,
    /// Offset of the path's coordinates, in canvas units (y up).
    pub position: Point,
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
    /// Uniform scale; 1 is normal size.
    pub scale: f64,
    /// Rotation in radians, counterclockwise.
    pub rotation: f64,
    /// Where the visible part starts, from 0 (path start) to 1 (end).
    pub start: f64,
    /// Where the visible part ends, from 0 (path start) to 1 (end).
    pub end: f64,
}

impl Path {
    /// A quadratic Bézier curve; each point can be fixed or follow a
    /// source.
    pub fn quadratic<M: crate::Model2d>(
        start: impl Into<crate::Input<Point, M>>,
        control: impl Into<crate::Input<Point, M>>,
        end: impl Into<crate::Input<Point, M>>,
    ) -> crate::Object<Self, M> {
        let inputs = (start.into(), control.into(), end.into());
        if let (Some(start), Some(control), Some(end)) = (
            inputs.0.as_fixed(),
            inputs.1.as_fixed(),
            inputs.2.as_fixed(),
        ) {
            return Self::from_bez_path(quadratic_geometry(
                *start, *control, *end,
            ));
        }
        let mut object = Self::from_bez_path(BezPath::new());
        object.on_insert(
            path!(Path.geometry).field.untyped(),
            Arc::new(move |scene, handle| {
                let geometry = crate::computed(
                    inputs.clone(),
                    |(start, control, end)| {
                        Arc::new(quadratic_geometry(
                            start, control, end,
                        ))
                    },
                );
                scene.bind(
                    handle.geometry(),
                    geometry,
                    std::convert::identity,
                );
            }),
        );
        object
    }

    /// Replace the geometry with a quadratic curve, unless it already is
    /// this curve. Style and visible range are kept.
    pub fn set_quadratic(
        &mut self,
        start: impl Into<Point>,
        control: impl Into<Point>,
        end: impl Into<Point>,
    ) {
        let (start, control, end) =
            (start.into(), control.into(), end.into());
        use kurbo::PathEl;
        if self.geometry.elements()
            == [PathEl::MoveTo(start), PathEl::QuadTo(control, end)]
        {
            return;
        }
        let geometry = Arc::make_mut(&mut self.geometry);
        geometry.truncate(0);
        geometry.move_to(start);
        geometry.quad_to(control, end);
    }

    /// A polyline through `points`, in canvas units (y up).
    pub fn new<M: crate::Model2d, P: Into<Point>>(
        points: impl IntoIterator<Item = P>,
    ) -> crate::Object<Self, M> {
        let mut geometry = BezPath::new();
        let mut points = points.into_iter();
        if let Some(point) = points.next() {
            geometry.move_to(point.into());
            for point in points {
                geometry.line_to(point.into());
            }
        }
        Self::from_bez_path(geometry)
    }

    /// A path with any geometry built with [`BezPath`].
    pub fn from_bez_path<M: crate::Model2d>(
        geometry: BezPath,
    ) -> crate::Object<Self, M> {
        crate::Object::new(Self {
            geometry: Arc::new(geometry),
            position: Point::ZERO,
            stroke: Color::WHITE,
            stroke_width: 1.0,
            line_cap: crate::Cap::Round,
            line_join: crate::Join::Round,
            opacity: 1.0,
            scale: 1.0,
            rotation: 0.0,
            start: 0.0,
            end: 1.0,
        })
    }
}

fn quadratic_geometry(
    start: Point,
    control: Point,
    end: Point,
) -> BezPath {
    let mut geometry = BezPath::new();
    geometry.move_to(start);
    geometry.quad_to(control, end);
    geometry
}
crate::object::stroke!(Path);
crate::object::stroke_options!(Path);

/// Identifies one [`Path`] in its scene.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct PathId(pub(crate) usize);
impl PathId {
    /// Its position in the scene's storage for this object type.
    pub fn index(self) -> usize {
        self.0
    }
}

impl<M: crate::Model2d> SceneObject<M> for Path {
    type Handle = Handle<Path>;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Self::Handle {
        let id = PathId(scene.model_mut().canvas_mut().paths.len());
        scene.model_mut().canvas_mut().paths.push(self);
        scene
            .model_mut()
            .canvas_mut()
            .order
            .push(DrawObject::Path(id));
        Handle::new(id)
    }
}

crate::capability::stored_in!(Path, PathId, paths);

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
crate::properties::object!(Path, PathId;
set position: Point => new(Path.position) shared;
set start: f64 => new(Path.start);
set end: f64 => new(Path.end);
set opacity: f64 => new(Path.opacity) shared;
set scale: f64 => new(Path.scale) shared;
set rotation: f64 => new(Path.rotation) shared;
get stroke: Color => new(Path.stroke) shared;
set stroke_width: f64 => new(Path.stroke_width) shared;
get line_cap: crate::Cap => discrete(Path.line_cap);
get line_join: crate::Join => discrete(Path.line_join);
set geometry: Arc<BezPath> => discrete(Path.geometry);
get position_x: f64 => new(Path.position.x) shared;
get position_y: f64 => new(Path.position.y) shared;
);
crate::capability::capabilities!(Path; Transformable, Strokable, Fadable);
