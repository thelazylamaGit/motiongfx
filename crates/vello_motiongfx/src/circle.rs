//! Filled circles and stroked rings with typed animation targets.

use crate::{
    Color, DrawObject, Graphics, Parent, Point, Position, Scene,
    SceneObject,
};
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
use vello::kurbo::{Affine, Circle as CircleShape, Stroke};
use vello::peniko::Fill;

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
    pub fn new(
        radius: impl Into<crate::Input<f64>>,
    ) -> crate::Object<Self> {
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

    pub(crate) fn transform(&self) -> Affine {
        Affine::translate((self.position.x, -self.position.y))
            * Affine::rotate(-self.rotation)
            * Affine::scale(self.scale)
    }

    pub(crate) fn render(
        &self,
        scene: &mut vello::Scene,
        parent: Affine,
    ) {
        self.render_with_opacity(scene, parent, 1.0);
    }
    pub(crate) fn render_with_opacity(
        &self,
        scene: &mut vello::Scene,
        parent: Affine,
        inherited: f32,
    ) {
        let opacity = self.opacity.clamp(0.0, 1.0) as f32 * inherited;
        let transform = parent * self.transform();
        if opacity == 0.0
            || self.radius <= 0.0
            || transform.determinant() == 0.0
        {
            return;
        }
        let shape = CircleShape::new(Point::ZERO, self.radius);
        if self.fill.components[3] > 0.0 {
            scene.fill(
                Fill::NonZero,
                transform,
                self.fill
                    .with_alpha(self.fill.components[3] * opacity),
                None,
                &shape,
            );
        }
        if self.stroke_width > 0.0 && self.stroke.components[3] > 0.0
        {
            scene.stroke(
                &Stroke::new(self.stroke_width),
                transform,
                self.stroke
                    .with_alpha(self.stroke.components[3] * opacity),
                None,
                &shape,
            );
        }
    }
}

crate::object::properties!(Circle;
    position: Point, radius: f64, fill: Color, opacity: f64,
    scale: f64, rotation: f64, stroke_width: f64,
);
crate::object::stroke!(Circle);
impl crate::Object<Circle> {
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

impl SceneObject for Circle {
    type Handle = CircleHandle;
    fn insert_into(self, scene: &mut Scene) -> Self::Handle {
        self.insert(scene, true)
    }
}
impl Circle {
    pub(crate) fn insert(
        self,
        scene: &mut Scene,
        draw: bool,
    ) -> CircleHandle {
        let id = CircleId(scene.graphics.circles.len());
        scene.graphics.circles.push(self);
        if draw {
            scene.graphics.order.push(DrawObject::Circle(id));
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

impl SubjectSource<CircleId, Circle> for Graphics {
    fn get_source(&self, id: CircleId) -> Option<&Circle> {
        self.circles.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: CircleId,
        f: impl FnOnce(&mut Circle) -> R,
    ) -> Option<R> {
        self.circles.get_mut(id.0).map(f)
    }
}
