//! Rectangle state, authoring and typed subject access.
use crate::*;
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
/// Rectangle centered on its position. Coordinates are logical canvas units,
/// positive y points up, and rotation is counterclockwise in radians.
/// Scale changes geometry without changing the stroke width.
#[derive(Clone)]
pub struct Rectangle {
    pub size: Size,
    pub position: Point,
    pub fill: Color,
    pub stroke: Color,
    pub stroke_width: f64,
    pub line_cap: crate::Cap,
    pub line_join: crate::Join,
    pub scale: f64,
    pub rotation: f64,
    pub radius: f64,
}

impl Default for Rectangle {
    fn default() -> Self {
        Self {
            size: Size::new(100.0, 100.0),
            position: Point::ZERO,
            fill: Color::WHITE,
            stroke: Color::TRANSPARENT,
            stroke_width: 0.0,
            line_cap: crate::Cap::Round,
            line_join: crate::Join::Round,
            scale: 1.0,
            rotation: 0.0,
            radius: 0.0,
        }
    }
}

impl Rectangle {
    /// Create initial rectangle state with the remaining properties defaulted.
    /// Register this value with `Scene::add` to obtain animation handles.
    pub fn new<M: Model2d>(
        width: f64,
        height: f64,
    ) -> Object<Self, M> {
        Object::new(Self {
            size: Size::new(width, height),
            ..Self::default()
        })
    }

    /// Create a rectangle from a `(width, height)` tuple, `Vec2`, or `Size`.
    pub fn from_size<M: Model2d>(
        size: impl Into<(f64, f64)>,
    ) -> Object<Self, M> {
        let (width, height) = size.into();
        Self::new(width, height)
    }
}

crate::object::properties!(Rectangle;
    size: Size, position: Point, fill: Color, radius: f64,
    scale: f64, rotation: f64, stroke_width: f64,
);
crate::object::stroke!(Rectangle);
crate::object::stroke_options!(Rectangle);

/// Typed subject identity; separate from authoring handles and sampled state.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct RectangleId(pub(crate) usize);

/// Setup-time property access, rather than the current property values.
#[derive(Clone, Copy)]
pub struct RectangleHandle {
    pub(crate) id: RectangleId,
    pub position: Position<RectangleId, Rectangle>,
    pub fill: Property<RectangleId, Rectangle, Color>,
    pub scale: Property<RectangleId, Rectangle, f64>,
    pub rotation: Property<RectangleId, Rectangle, f64>,
    pub size: Property<RectangleId, Rectangle, Size>,
    pub radius: Property<RectangleId, Rectangle, f64>,
    pub stroke: Property<RectangleId, Rectangle, Color>,
    pub stroke_width: Property<RectangleId, Rectangle, f64>,
    pub line_cap: Property<RectangleId, Rectangle, crate::Cap>,
    pub line_join: Property<RectangleId, Rectangle, crate::Join>,
}

impl<M: Model2d> SubjectSource<RectangleId, Rectangle> for World<M> {
    fn get_source(&self, id: RectangleId) -> Option<&Rectangle> {
        self.model().graphics().rectangles.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: RectangleId,
        f: impl FnOnce(&mut Rectangle) -> R,
    ) -> Option<R> {
        self.model_mut()
            .graphics_mut()
            .rectangles
            .get_mut(id.0)
            .map(f)
    }
}

fn insert_rectangle<M: Model2d>(
    scene: &mut motiongfx_authoring::Scene<M>,
    rectangle: Rectangle,
) -> RectangleHandle {
    let id = RectangleId(
        scene.model_mut().graphics_mut().rectangles.len(),
    );
    scene.model_mut().graphics_mut().rectangles.push(rectangle);
    scene
        .model_mut()
        .graphics_mut()
        .order
        .push(DrawObject::Rectangle(id));
    RectangleHandle {
        id,
        position: Position::new(
            id,
            path!(Rectangle.position),
            path!(Rectangle.position.x),
            path!(Rectangle.position.y),
        ),
        fill: Property::new(id, path!(Rectangle.fill)),
        scale: Property::new(id, path!(Rectangle.scale)),
        rotation: Property::new(id, path!(Rectangle.rotation)),
        size: Property::new(id, path!(Rectangle.size)),
        radius: Property::new(id, path!(Rectangle.radius)),
        stroke: Property::new(id, path!(Rectangle.stroke)),
        line_cap: Property::discrete(id, path!(Rectangle.line_cap)),
        line_join: Property::discrete(id, path!(Rectangle.line_join)),
        stroke_width: Property::new(
            id,
            path!(Rectangle.stroke_width),
        ),
    }
}

impl RectangleId {
    pub fn index(self) -> usize {
        self.0
    }
}
impl RectangleHandle {
    pub fn id(self) -> RectangleId {
        self.id
    }
}
impl<M: Model2d> SceneObject<M> for Rectangle {
    type Handle = RectangleHandle;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> RectangleHandle {
        insert_rectangle(scene, self)
    }
}
