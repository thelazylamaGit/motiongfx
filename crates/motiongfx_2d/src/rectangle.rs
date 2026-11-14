//! [`Rectangle`]: rectangles, optionally with rounded corners.
use crate::*;
use motiongfx::prelude::SubjectSource;
/// A rectangle centred on its position. Create one with [`Rectangle::new`].
/// Scale changes its size but not its stroke width.
#[derive(Clone)]
pub struct Rectangle {
    /// Width and height in canvas units.
    pub size: Size,
    /// Centre, in canvas units (y up).
    pub position: Point,
    /// Fill colour.
    pub fill: Color,
    /// Outline colour.
    pub stroke: Color,
    /// Outline width in canvas units.
    pub stroke_width: f64,
    /// How open stroke ends and dash ends are drawn.
    pub line_cap: crate::Cap,
    /// How stroke corners are drawn.
    pub line_join: crate::Join,
    /// Uniform scale; 1 is normal size.
    pub scale: f64,
    /// Rotation in radians, counterclockwise.
    pub rotation: f64,
    /// Corner radius in canvas units; 0 for sharp corners.
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
    /// A white `width` by `height` rectangle; set other fields with the
    /// builder methods, then pass it to `scene.add`.
    pub fn new<M: Model2d>(
        width: f64,
        height: f64,
    ) -> Object<Self, M> {
        Object::new(Self {
            size: Size::new(width, height),
            ..Self::default()
        })
    }

    /// A white rectangle from a `(width, height)` tuple, `Vec2` or `Size`.
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

/// Identifies one [`Rectangle`] in its scene.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct RectangleId(pub(crate) usize);

/// A [`Rectangle`] in a scene, returned by `scene.add`. Its
/// methods return fields to animate or for other objects to follow.
#[derive(Clone, Copy, Debug)]
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
    /// Its position in the scene's storage for this object type.
    pub fn index(self) -> usize {
        self.0
    }
}
impl RectangleHandle {
    /// The object's ID.
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

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
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
