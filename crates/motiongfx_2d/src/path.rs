//! Stroked paths. Geometry is shared, while small reveal/style state is sampled.

use crate::{Color, DrawObject, Point, Position, SceneObject};
pub use kurbo::BezPath;
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
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
    pub position: Position<PathId, Path>,
    pub stroke: Property<PathId, Path, Color>,
    pub stroke_width: Property<PathId, Path, f64>,
    pub line_cap: Property<PathId, Path, crate::Cap>,
    pub line_join: Property<PathId, Path, crate::Join>,
    pub opacity: Property<PathId, Path, f64>,
    pub scale: Property<PathId, Path, f64>,
    pub rotation: Property<PathId, Path, f64>,
    pub start: Property<PathId, Path, f64>,
    pub end: Property<PathId, Path, f64>,
    pub geometry: Property<PathId, Path, Arc<BezPath>>,
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
            position: Position::new(
                id,
                path!(Path.position),
                path!(Path.position.x),
                path!(Path.position.y),
            ),
            stroke: Property::new(id, path!(Path.stroke)),
            line_cap: Property::discrete(id, path!(Path.line_cap)),
            line_join: Property::discrete(id, path!(Path.line_join)),
            stroke_width: Property::new(id, path!(Path.stroke_width)),
            opacity: Property::new(id, path!(Path.opacity)),
            scale: Property::new(id, path!(Path.scale)),
            rotation: Property::new(id, path!(Path.rotation)),
            start: Property::new(id, path!(Path.start)),
            end: Property::new(id, path!(Path.end)),
            geometry: Property::discrete(id, path!(Path.geometry)),
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
