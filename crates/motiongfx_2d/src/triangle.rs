//! [`Triangle`]: a filled arrowhead, optionally with rounded corners.
use crate::rounded_triangle::{self, Geometry};
use crate::{
    Cap, Color, DrawObject, Handle, Join, Model2d, Object, Point,
    SceneObject,
};
use kurbo::Affine;

/// A filled arrowhead facing +x, with its apex at `position`. Create one
/// with [`Triangle::new`]; it also works as a line tip.
#[derive(Clone)]
#[non_exhaustive]
pub struct Triangle {
    /// Length from the apex to the base, in canvas units.
    pub size: f64,
    /// Full opening angle at the apex, in radians.
    pub angle: f64,
    /// Corner radius in canvas units, reduced if it doesn't fit.
    pub radius: f64,
    /// Where the apex is, in canvas units (y up).
    pub position: Point,
    /// Rotation in radians, counterclockwise.
    pub rotation: f64,
    /// Uniform scale; 1 is normal size.
    pub scale: f64,
    /// 0 (invisible) to 1 (opaque).
    pub opacity: f64,
    /// Fill colour.
    pub fill: Color,
    /// Outline colour.
    pub stroke: Color,
    /// Outline width in canvas units.
    pub stroke_width: f64,
    /// How outline ends are drawn.
    pub line_cap: Cap,
    /// How outline corners are drawn.
    pub line_join: Join,
}

impl Default for Triangle {
    fn default() -> Self {
        Self {
            size: 24.0,
            angle: std::f64::consts::FRAC_PI_3,
            radius: 0.0,
            position: Point::ZERO,
            rotation: 0.0,
            scale: 1.0,
            opacity: 1.0,
            fill: Color::WHITE,
            stroke: Color::TRANSPARENT,
            stroke_width: 0.0,
            line_cap: Cap::Round,
            line_join: Join::Round,
        }
    }
}

impl Triangle {
    /// A white triangle 24 units long with a 60° opening; set other fields
    /// with the builder methods.
    pub fn new<M: Model2d>() -> Object<Self, M> {
        Object::new(Self::default())
    }
    /// The outline in local coordinates, with rounded corners if `radius`
    /// is set.
    pub fn geometry(&self) -> Geometry {
        rounded_triangle::rounded(
            crate::wedge::vertices(self.size, self.angle),
            self.radius,
        )
    }
    /// Where the triangle's local coordinates sit on the canvas.
    pub fn transform(&self) -> Affine {
        crate::wedge::transform(
            self.position,
            self.rotation,
            self.scale,
        )
    }
    /// Add the triangle and draw it in insertion order.
    pub(crate) fn insert<M: Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Handle<Self> {
        let handle = self.insert_tip(scene);
        scene
            .model_mut()
            .canvas_mut()
            .order
            .push(DrawObject::Triangle(handle.id));
        handle
    }
    /// Add the triangle without drawing it; its line draws it as a tip.
    pub(crate) fn insert_tip<M: Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Handle<Self> {
        let triangles = &mut scene.model_mut().canvas_mut().triangles;
        triangles.push(self);
        Handle::new(TriangleId(triangles.len() - 1))
    }
}

/// Identifies one [`Triangle`] in its scene.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct TriangleId(pub(crate) usize);
impl TriangleId {
    /// Its position in the scene's storage for triangles.
    pub fn index(self) -> usize {
        self.0
    }
}

impl<M: Model2d> SceneObject<M> for Triangle {
    type Handle = Handle<Self>;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Handle<Self> {
        self.insert(scene)
    }
}

crate::capability::stored_in!(Triangle, TriangleId, triangles);
crate::object::stroke!(Triangle);
crate::object::stroke_options!(Triangle);

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
crate::properties::object!(Triangle, TriangleId;
set size: f64 => new(Triangle.size);
set angle: f64 => new(Triangle.angle);
set radius: f64 => new(Triangle.radius);
set position: Point => new(Triangle.position) shared;
set rotation: f64 => new(Triangle.rotation) shared;
set scale: f64 => new(Triangle.scale) shared;
set opacity: f64 => new(Triangle.opacity) shared;
set fill: Color => new(Triangle.fill) shared;
get stroke: Color => new(Triangle.stroke) shared;
set stroke_width: f64 => new(Triangle.stroke_width) shared;
get line_cap: crate::Cap => discrete(Triangle.line_cap);
get line_join: crate::Join => discrete(Triangle.line_join);
get position_x: f64 => new(Triangle.position.x) shared;
get position_y: f64 => new(Triangle.position.y) shared;
);
crate::capability::capabilities!(Triangle; Transformable, Fillable, Strokable, Fadable);
