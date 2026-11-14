//! Filled circles and stroked rings with typed animation targets.

use crate::{Color, DrawObject, Parent, Point, SceneObject};
use kurbo::Affine;
use motiongfx::prelude::SubjectSource;

/// A circle with a local radius in canvas units. Scale transforms both the
/// circle and its stroke, including children anchored with `relative_to`.
/// Full circles have no caps or sharp joins. If open arcs/dashes are added,
/// expose their meaningful stroke options then, rather than storing unused
/// cap/join properties on today's circle.
#[derive(Clone)]
pub struct Circle {
    pub radius: f64,
    pub position: Point,
    pub fill: Color,
    pub stroke: Color,
    pub stroke_width: f64,
    pub opacity: f64,
    pub scale: f64,
    pub rotation: f64,
    pub(crate) parent: Option<Parent>,
}

impl Circle {
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

    pub fn transform(&self) -> Affine {
        Affine::translate((self.position.x, -self.position.y))
            * Affine::rotate(-self.rotation)
            * Affine::scale(self.scale)
    }
}

crate::object::properties!(Circle;
    position: Point, radius: f64, fill: Color, opacity: f64,
    scale: f64, rotation: f64, stroke_width: f64,
);
crate::object::stroke!(Circle);
impl<M: crate::Model2d> crate::Object<Circle, M> {
    /// Inherit the parent's transform; style and opacity remain independent.
    pub fn relative_to(mut self, parent: impl Into<Parent>) -> Self {
        self.parent = Some(parent.into());
        self
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct CircleId(pub(crate) usize);
impl CircleId {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct CircleHandle {
    pub(crate) id: CircleId,
}
impl CircleHandle {
    pub fn id(self) -> CircleId {
        self.id
    }
}

impl<M: crate::Model2d> SceneObject<M> for Circle {
    type Handle = CircleHandle;
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
    ) -> CircleHandle {
        let handle = self.insert_tip(scene);
        let graphics = scene.model_mut().graphics_mut();
        graphics.order.push(DrawObject::Circle(handle.id));
        handle
    }
    /// Add the circle without drawing it; its line draws it as a tip.
    pub(crate) fn insert_tip<M: crate::Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> CircleHandle {
        let circles = &mut scene.model_mut().graphics_mut().circles;
        circles.push(self);
        CircleHandle {
            id: CircleId(circles.len() - 1),
        }
    }
}

impl<M: crate::Model2d> SubjectSource<CircleId, Circle>
    for motiongfx_authoring::World<M>
{
    fn get_source(&self, id: CircleId) -> Option<&Circle> {
        self.model().graphics().circles.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: CircleId,
        f: impl FnOnce(&mut Circle) -> R,
    ) -> Option<R> {
        self.model_mut().graphics_mut().circles.get_mut(id.0).map(f)
    }
}

// Fixed field identity is generated locally; no accessor metadata is stored in
// each handle. See properties.rs and motiongfx_authoring::Target.
crate::properties::fields!(fields, Circle, CircleId;
radius: f64 => new(Circle.radius);
fill: Color => new(Circle.fill);
stroke: Color => new(Circle.stroke);
stroke_width: f64 => new(Circle.stroke_width);
opacity: f64 => new(Circle.opacity);
scale: f64 => new(Circle.scale);
rotation: f64 => new(Circle.rotation);
position: Point => new(Circle.position);
position_x: f64 => new(Circle.position.x);
position_y: f64 => new(Circle.position.y);
);

crate::properties::handle_methods!(CircleHandle,Circle,CircleId;
radius: f64;
fill: Color;
stroke: Color;
stroke_width: f64;
opacity: f64;
scale: f64;
rotation: f64;
position: Point;
position_x: f64;
position_y: f64;
);

crate::properties::transform_methods!(CircleHandle, Circle, CircleId);

crate::properties::fill_methods!(CircleHandle, Circle, CircleId);
