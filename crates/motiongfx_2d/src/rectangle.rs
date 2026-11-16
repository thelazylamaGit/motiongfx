//! [`Rectangle`]: rectangles, optionally with rounded corners.
use crate::*;
/// A rectangle centred on its position. Create one with [`Rectangle::new`].
/// Scale changes its size but not its stroke width.
#[derive(Clone)]
#[non_exhaustive]
pub struct Rectangle {
    /// Width and height in canvas units.
    pub size: Size,
    /// Centre, in canvas units (y up).
    pub position: Point,
    /// Fill colour.
    pub fill: Color,
    /// 0 (invisible) to 1 (opaque); multiplies the fill and outline alpha.
    pub opacity: f64,
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
            opacity: 1.0,
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

    /// Where the rectangle's local coordinates sit on the canvas. Scale is
    /// applied to the size instead, so strokes keep their width.
    pub fn transform(&self) -> Affine {
        Affine::translate((self.position.x, -self.position.y))
            * Affine::rotate(-self.rotation)
    }

    /// A white rectangle from a `(width, height)` tuple, `Vec2` or `Size`.
    pub fn from_size<M: Model2d>(
        size: impl Into<(f64, f64)>,
    ) -> Object<Self, M> {
        let (width, height) = size.into();
        Self::new(width, height)
    }
}

crate::object::stroke!(Rectangle);
crate::object::stroke_options!(Rectangle);

/// Identifies one [`Rectangle`] in its scene.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct RectangleId(pub(crate) usize);

crate::capability::stored_in!(Rectangle, RectangleId, rectangles);

fn insert_rectangle<M: Model2d>(
    scene: &mut motiongfx_authoring::Scene<M>,
    rectangle: Rectangle,
) -> Handle<Rectangle> {
    let id =
        RectangleId(scene.model_mut().canvas_mut().rectangles.len());
    scene.model_mut().canvas_mut().rectangles.push(rectangle);
    scene
        .model_mut()
        .canvas_mut()
        .order
        .push(DrawObject::Rectangle(id));
    Handle::new(id)
}

impl RectangleId {
    /// Its position in the scene's storage for this object type.
    pub fn index(self) -> usize {
        self.0
    }
}
impl<M: Model2d> SceneObject<M> for Rectangle {
    type Handle = Handle<Rectangle>;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Handle<Rectangle> {
        insert_rectangle(scene, self)
    }
}

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
crate::properties::object!(Rectangle, RectangleId;
set size: Size => new(Rectangle.size);
set position: Point => new(Rectangle.position) shared;
set fill: Color => new(Rectangle.fill) shared;
set opacity: f64 => new(Rectangle.opacity) shared;
set radius: f64 => new(Rectangle.radius);
set scale: f64 => new(Rectangle.scale) shared;
set rotation: f64 => new(Rectangle.rotation) shared;
get stroke: Color => new(Rectangle.stroke) shared;
set stroke_width: f64 => new(Rectangle.stroke_width) shared;
get line_cap: crate::Cap => discrete(Rectangle.line_cap);
get line_join: crate::Join => discrete(Rectangle.line_join);
get position_x: f64 => new(Rectangle.position.x) shared;
get position_y: f64 => new(Rectangle.position.y) shared;
);
crate::capability::capabilities!(Rectangle; Transformable, Fillable, Strokable, Fadable);
