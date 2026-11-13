//! Stroked paths. Geometry is shared, while small reveal/style state is sampled.

use crate::{Color, DrawObject, Point, SceneObject};
pub use kurbo::BezPath;
use motiongfx::prelude::{SubjectSource, path};
use std::sync::Arc;

#[derive(Clone)]
pub struct Path {
    pub geometry: Arc<BezPath>,
    pub position: Point,
    pub stroke: Color,
    pub stroke_width: f64,
    pub line_cap: crate::Cap,
    pub line_join: crate::Join,
    pub opacity: f64,
    pub scale: f64,
    pub rotation: f64,
    pub start: f64,
    pub end: f64,
}

impl Path {
    /// Exact quadratic geometry, rather than sampling a polyline.
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
                    handle.geometry,
                    geometry,
                    std::convert::identity,
                );
            }),
        );
        object
    }

    /// Change a quadratic only when its control points differ. Fixed geometry
    /// is reused through pauses and repeated samples; style/reveal stay intact.
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

    /// Create a polyline from canvas points (positive y points up).
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

    /// Use arbitrary line, quadratic or cubic geometry generated in Rust.
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
crate::object::properties!(Path;
    position: Point, start: f64, end: f64, opacity: f64,
    scale: f64, rotation: f64, stroke_width: f64, geometry: Arc<BezPath>,
);
crate::object::stroke!(Path);
crate::object::stroke_options!(Path);

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct PathId(pub(crate) usize);
impl PathId {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct PathHandle {
    pub(crate) id: PathId,
    pub position: path_position,
    pub stroke: fields::stroke::Target,
    pub stroke_width: fields::stroke_width::Target,
    pub line_cap: fields::line_cap::Target,
    pub line_join: fields::line_join::Target,
    pub opacity: fields::opacity::Target,
    pub scale: fields::scale::Target,
    pub rotation: fields::rotation::Target,
    pub start: fields::start::Target,
    pub end: fields::end::Target,
    pub geometry: fields::geometry::Target,
}
impl PathHandle {
    pub fn id(self) -> PathId {
        self.id
    }
}

impl<M: crate::Model2d> SceneObject<M> for Path {
    type Handle = PathHandle;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Self::Handle {
        let id = PathId(scene.model_mut().graphics_mut().paths.len());
        scene.model_mut().graphics_mut().paths.push(self);
        scene
            .model_mut()
            .graphics_mut()
            .order
            .push(DrawObject::Path(id));
        PathHandle {
            id,
            position: path_position_axes(id),
            stroke: motiongfx_authoring::Target::new(id),
            line_cap: motiongfx_authoring::Target::new(id),
            line_join: motiongfx_authoring::Target::new(id),
            stroke_width: motiongfx_authoring::Target::new(id),
            opacity: motiongfx_authoring::Target::new(id),
            scale: motiongfx_authoring::Target::new(id),
            rotation: motiongfx_authoring::Target::new(id),
            start: motiongfx_authoring::Target::new(id),
            end: motiongfx_authoring::Target::new(id),
            geometry: motiongfx_authoring::Target::new(id),
        }
    }
}

impl<M: crate::Model2d> SubjectSource<PathId, Path>
    for motiongfx_authoring::World<M>
{
    fn get_source(&self, id: PathId) -> Option<&Path> {
        self.model().graphics().paths.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: PathId,
        f: impl FnOnce(&mut Path) -> R,
    ) -> Option<R> {
        self.model_mut().graphics_mut().paths.get_mut(id.0).map(f)
    }
}

// Fixed field identity is generated locally; no accessor metadata is stored in
// each handle. See properties.rs and motiongfx_authoring::Target.
crate::properties::fields!(fields, Path, PathId;
stroke: Color => new(Path.stroke);
stroke_width: f64 => new(Path.stroke_width);
line_cap: crate::Cap => discrete(Path.line_cap);
line_join: crate::Join => discrete(Path.line_join);
opacity: f64 => new(Path.opacity);
scale: f64 => new(Path.scale);
rotation: f64 => new(Path.rotation);
start: f64 => new(Path.start);
end: f64 => new(Path.end);
geometry: Arc<BezPath> => discrete(Path.geometry);
position: Point => new(Path.position);
position_x: f64 => new(Path.position.x);
position_y: f64 => new(Path.position.y);
);
#[allow(non_camel_case_types)]
pub type path_position = crate::Position<
    PathId,
    Path,
    fields::position::Target,
    fields::position_x::Target,
    fields::position_y::Target,
>;
pub fn path_position_axes(id: PathId) -> path_position {
    crate::Position::from_targets(
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
    )
}
