//! Rectangle state, authoring and typed subject access.
use crate::*;
use motiongfx::prelude::SubjectSource;
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
    RectangleHandle { id }
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

// Fixed field identity is generated locally; no accessor metadata is stored in
// each handle. See properties.rs and motiongfx_authoring::Target.
crate::properties::fields!(fields, Rectangle, RectangleId;
fill: Color => new(Rectangle.fill);
scale: f64 => new(Rectangle.scale);
rotation: f64 => new(Rectangle.rotation);
size: Size => new(Rectangle.size);
radius: f64 => new(Rectangle.radius);
stroke: Color => new(Rectangle.stroke);
stroke_width: f64 => new(Rectangle.stroke_width);
line_cap: crate::Cap => discrete(Rectangle.line_cap);
line_join: crate::Join => discrete(Rectangle.line_join);
position: Point => new(Rectangle.position);
position_x: f64 => new(Rectangle.position.x);
position_y: f64 => new(Rectangle.position.y);
);
#[allow(non_camel_case_types)]
pub type rectangle_position = crate::Position<
    RectangleId,
    Rectangle,
    fields::position::Target,
    fields::position_x::Target,
    fields::position_y::Target,
>;
pub fn rectangle_position_axes(
    id: RectangleId,
) -> rectangle_position {
    crate::Position::from_targets(
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
    )
}

crate::properties::handle_methods!(RectangleHandle,Rectangle,RectangleId;
fill: Color;
scale: f64;
rotation: f64;
size: Size;
radius: f64;
stroke: Color;
stroke_width: f64;
line_cap: crate::Cap;
line_join: crate::Join;
position: Point;
position_x: f64;
position_y: f64;
);

crate::properties::transform_methods!(
    RectangleHandle,
    Rectangle,
    RectangleId
);

crate::properties::fill_methods!(
    RectangleHandle,
    Rectangle,
    RectangleId
);
