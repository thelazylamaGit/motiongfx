//! Filled circles and stroked rings with typed animation targets.

use crate::{
    Color, DrawObject, Parent, Point, Position, SceneObject,
};
use kurbo::Affine;
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;

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
    pub position: Position<CircleId, Circle>,
    pub radius: Property<CircleId, Circle, f64>,
    pub fill: Property<CircleId, Circle, Color>,
    pub stroke: Property<CircleId, Circle, Color>,
    pub stroke_width: Property<CircleId, Circle, f64>,
    pub opacity: Property<CircleId, Circle, f64>,
    pub scale: Property<CircleId, Circle, f64>,
    pub rotation: Property<CircleId, Circle, f64>,
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
        self.insert(scene, true)
    }
}
impl Circle {
    pub(crate) fn insert<M: crate::Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
        draw: bool,
    ) -> CircleHandle {
        let id =
            CircleId(scene.model_mut().graphics_mut().circles.len());
        scene.model_mut().graphics_mut().circles.push(self);
        if draw {
            scene
                .model_mut()
                .graphics_mut()
                .order
                .push(DrawObject::Circle(id));
        }
        CircleHandle {
            id,
            position: Position::new(
                id,
                path!(Circle.position),
                path!(Circle.position.x),
                path!(Circle.position.y),
            ),
            radius: Property::new(id, path!(Circle.radius)),
            fill: Property::new(id, path!(Circle.fill)),
            stroke: Property::new(id, path!(Circle.stroke)),

            stroke_width: Property::new(
                id,
                path!(Circle.stroke_width),
            ),
            opacity: Property::new(id, path!(Circle.opacity)),
            scale: Property::new(id, path!(Circle.scale)),
            rotation: Property::new(id, path!(Circle.rotation)),
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
