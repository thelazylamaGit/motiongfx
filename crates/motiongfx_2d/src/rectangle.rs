//! [`Rectangle`]: rectangles, optionally with rounded corners.
use crate::*;
/// A rectangle placed by its [`anchor`](Self::anchor), its centre unless
/// set otherwise. Create one with [`Rectangle::new`]. Scale changes its
/// size but not its stroke width.
#[derive(Clone)]
#[non_exhaustive]
pub struct Rectangle {
    /// Width and height in canvas units.
    pub size: Size,
    /// Position, scale and rotation. The position is where the anchor sits,
    /// in canvas units (y up).
    pub transform: crate::Transform,
    /// The point of the rectangle that its position marks and that it
    /// rotates and scales about, as a fraction of its half-size: `(0, 0)` is
    /// the centre, `(0, -1)` the middle of the bottom edge and `(1, 1)` the
    /// top-right corner (y up). Values beyond ±1 put the point outside the
    /// rectangle. Children attached with `relative_to` hang off this point.
    /// See [`anchor`](crate::anchor) for named points.
    pub anchor: Vec2,
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
    /// Corner radius in canvas units; 0 for sharp corners.
    pub radius: f64,
}

impl Default for Rectangle {
    fn default() -> Self {
        Self {
            size: Size::new(100.0, 100.0),
            transform: crate::Transform::default(),
            anchor: anchor::CENTER,
            fill: Color::WHITE,
            opacity: 1.0,
            stroke: Color::TRANSPARENT,
            stroke_width: 0.0,
            line_cap: crate::Cap::Round,
            line_join: crate::Join::Round,
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
        self.transform.without_scale()
    }

    /// The rectangle in its own coordinates, ready to draw with
    /// [`transform`](Self::transform): scaled, and placed so its anchor is
    /// at the origin. Like `transform`, y points down.
    pub fn local_rect(&self) -> Rect {
        let half = Vec2::new(self.size.width, self.size.height)
            * (self.transform.scale / 2.0);
        // The anchor's y points up, as positions do.
        let shift = Vec2::new(
            self.anchor.x * half.x,
            -self.anchor.y * half.y,
        );
        Rect::new(-half.x, -half.y, half.x, half.y) - shift
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
transform(Rectangle.transform);
set size: Size => new(Rectangle.size);
get width: f64 => new(Rectangle.size.width);
get height: f64 => new(Rectangle.size.height);
/// Set the [`anchor`](Rectangle::anchor): a fixed point such as
/// [`anchor::BOTTOM`](crate::anchor::BOTTOM), or a signal, computed value
/// or property for it to follow.
set anchor: Vec2 => new(Rectangle.anchor);
set fill: Color => new(Rectangle.fill) shared;
set opacity: f64 => new(Rectangle.opacity) shared;
set radius: f64 => new(Rectangle.radius);
get stroke: Color => new(Rectangle.stroke) shared;
set stroke_width: f64 => new(Rectangle.stroke_width) shared;
get line_cap: crate::Cap => discrete(Rectangle.line_cap);
get line_join: crate::Join => discrete(Rectangle.line_join);
);
/// Named [`Rectangle::anchor`] points: which point of a rectangle its
/// position marks and it turns and scales about.
pub mod anchor {
    use crate::Vec2;

    /// The centre, the default.
    pub const CENTER: Vec2 = Vec2::new(0.0, 0.0);
    /// The middle of the top edge.
    pub const TOP: Vec2 = Vec2::new(0.0, 1.0);
    /// The middle of the bottom edge.
    pub const BOTTOM: Vec2 = Vec2::new(0.0, -1.0);
    /// The middle of the left edge.
    pub const LEFT: Vec2 = Vec2::new(-1.0, 0.0);
    /// The middle of the right edge.
    pub const RIGHT: Vec2 = Vec2::new(1.0, 0.0);
    /// The top-left corner.
    pub const TOP_LEFT: Vec2 = Vec2::new(-1.0, 1.0);
    /// The top-right corner.
    pub const TOP_RIGHT: Vec2 = Vec2::new(1.0, 1.0);
    /// The bottom-left corner.
    pub const BOTTOM_LEFT: Vec2 = Vec2::new(-1.0, -1.0);
    /// The bottom-right corner.
    pub const BOTTOM_RIGHT: Vec2 = Vec2::new(1.0, -1.0);
}

crate::capability::capabilities!(Rectangle; Transformable, Fillable, Strokable, Fadable);
