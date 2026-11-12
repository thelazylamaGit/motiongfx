//! Stroked paths. Geometry is shared, while small reveal/style state is sampled.

use crate::{
    Color, DrawObject, Graphics, Point, Position, Scene, SceneObject,
};
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
use std::sync::Arc;
pub use vello::kurbo::BezPath;

#[derive(Clone)]
pub struct Path {
    pub geometry: Arc<BezPath>,
    pub position: Point,
    pub stroke: Color,
    pub stroke_width: f64,
    pub opacity: f64,
    pub scale: f64,
    pub rotation: f64,
    pub start: f64,
    pub end: f64,
}

impl Path {
    /// Exact quadratic geometry, rather than sampling a polyline.
    pub fn quadratic(
        start: impl Into<Point>,
        control: impl Into<Point>,
        end: impl Into<Point>,
    ) -> Self {
        let mut path = Self::from_bez_path(BezPath::new());
        path.set_quadratic(start, control, end);
        path
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
        use vello::kurbo::PathEl;
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
    pub fn new<P: Into<Point>>(
        points: impl IntoIterator<Item = P>,
    ) -> Self {
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
    pub fn from_bez_path(geometry: BezPath) -> Self {
        Self {
            geometry: Arc::new(geometry),
            position: Point::ZERO,
            stroke: Color::WHITE,
            stroke_width: 1.0,
            opacity: 1.0,
            scale: 1.0,
            rotation: 0.0,
            start: 0.0,
            end: 1.0,
        }
    }
    pub fn position(mut self, position: impl Into<Point>) -> Self {
        self.position = position.into();
        self
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
    pub fn start(mut self, start: f64) -> Self {
        self.start = start;
        self
    }
    pub fn end(mut self, end: f64) -> Self {
        self.end = end;
        self
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct PathId(pub(crate) usize);

#[derive(Clone, Copy)]
pub struct PathHandle {
    pub(crate) id: PathId,
    pub position: Position<PathId, Path>,
    pub stroke: Property<PathId, Path, Color>,
    pub stroke_width: Property<PathId, Path, f64>,
    pub opacity: Property<PathId, Path, f64>,
    pub scale: Property<PathId, Path, f64>,
    pub rotation: Property<PathId, Path, f64>,
    pub start: Property<PathId, Path, f64>,
    pub end: Property<PathId, Path, f64>,
}

impl SceneObject for Path {
    type Handle = PathHandle;
    fn insert_into(self, scene: &mut Scene) -> Self::Handle {
        let id = PathId(scene.graphics.paths.len());
        scene.graphics.paths.push(self);
        scene.graphics.order.push(DrawObject::Path(id));
        PathHandle {
            id,
            position: Position::new(
                id,
                path!(Path.position),
                path!(Path.position.x),
                path!(Path.position.y),
            ),
            stroke: Property::new(id, path!(Path.stroke)),
            stroke_width: Property::new(id, path!(Path.stroke_width)),
            opacity: Property::new(id, path!(Path.opacity)),
            scale: Property::new(id, path!(Path.scale)),
            rotation: Property::new(id, path!(Path.rotation)),
            start: Property::new(id, path!(Path.start)),
            end: Property::new(id, path!(Path.end)),
        }
    }
}

impl SubjectSource<PathId, Path> for Graphics {
    fn get_source(&self, id: PathId) -> Option<&Path> {
        self.paths.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: PathId,
        f: impl FnOnce(&mut Path) -> R,
    ) -> Option<R> {
        self.paths.get_mut(id.0).map(f)
    }
}
