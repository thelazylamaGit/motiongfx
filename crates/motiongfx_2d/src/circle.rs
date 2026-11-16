//! [`Circle`]: filled circles and, with a transparent fill, rings.
use crate::Handle;

use crate::{Color, DrawObject, Parent, Point, SceneObject};
use kurbo::Affine;

/// A circle. Create one with [`Circle::new`]; for a ring, use a
/// transparent fill and a stroke. Scale also scales the stroke and any
/// objects attached with `relative_to`.
#[derive(Clone)]
pub struct Circle {
    /// Radius in canvas units.
    pub radius: f64,
    /// Centre, in canvas units (y up).
    pub position: Point,
    /// Fill colour.
    pub fill: Color,
    /// Outline colour.
    pub stroke: Color,
    /// Outline width in canvas units.
    pub stroke_width: f64,
    /// 0 (invisible) to 1 (opaque).
    pub opacity: f64,
    /// Uniform scale; 1 is normal size.
    pub scale: f64,
    /// Rotation in radians, counterclockwise.
    pub rotation: f64,
    pub(crate) parent: Option<Parent>,
}

impl Circle {
    /// A white circle of `radius` (a fixed value or a source to follow).
    pub fn new<M: crate::Model2d>(
        radius: impl Into<crate::Input<f64, M>>,
    ) -> crate::Object<Self, M> {
        crate::Object::new(Self {
            radius: 0.0,
            position: Point::ZERO,
            fill: Color::WHITE,
            stroke: Color::TRANSPARENT,
            stroke_width: 0.0,
            opacity: 1.0,
            scale: 1.0,
            rotation: 0.0,
            parent: None,
        })
        .radius(radius)
    }

    /// Where the circle's local coordinates sit, relative to its parent.
    pub fn transform(&self) -> Affine {
        Affine::translate((self.position.x, -self.position.y))
            * Affine::rotate(-self.rotation)
            * Affine::scale(self.scale)
    }
}

crate::object::stroke!(Circle);
impl<M: crate::Model2d> crate::Object<Circle, M> {
    /// Attach to a rectangle or circle already in the scene: `position`
    /// becomes an offset from it, and the circle moves, turns and scales
    /// with it. Colour and opacity stay independent.
    pub fn relative_to(mut self, parent: impl Into<Parent>) -> Self {
        self.parent = Some(parent.into());
        self
    }
}

/// Identifies one [`Circle`] in its scene.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct CircleId(pub(crate) usize);
impl CircleId {
    /// Its position in the scene's storage for this object type.
    pub fn index(self) -> usize {
        self.0
    }
}

impl<M: crate::Model2d> SceneObject<M> for Circle {
    type Handle = Handle<Circle>;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Self::Handle {
        self.insert(scene)
    }
}
impl Circle {
    /// Add the circle and draw it in insertion order.
    pub(crate) fn insert<M: crate::Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Handle<Circle> {
        let handle = self.insert_tip(scene);
        let graphics = scene.model_mut().canvas_mut();
        graphics.order.push(DrawObject::Circle(handle.id));
        handle
    }
    /// Add the circle without drawing it; its line draws it as a tip.
    pub(crate) fn insert_tip<M: crate::Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Handle<Circle> {
        let circles = &mut scene.model_mut().canvas_mut().circles;
        circles.push(self);
        Handle::new(CircleId(circles.len() - 1))
    }
}

crate::capability::stored_in!(Circle, CircleId, circles);

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
crate::properties::object!(Circle, CircleId;
set radius: f64 => new(Circle.radius);
set fill: Color => new(Circle.fill) shared;
get stroke: Color => new(Circle.stroke) shared;
set stroke_width: f64 => new(Circle.stroke_width) shared;
set opacity: f64 => new(Circle.opacity) shared;
set scale: f64 => new(Circle.scale) shared;
set rotation: f64 => new(Circle.rotation) shared;
set position: Point => new(Circle.position) shared;
get position_x: f64 => new(Circle.position.x) shared;
get position_y: f64 => new(Circle.position.y) shared;
);
crate::capability::capabilities!(Circle; Transformable, Fillable, Strokable, Fadable);
