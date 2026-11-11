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
    pub fn new(radius: f64) -> Self {
        Self {
            radius,
            position: Point::ZERO,
            fill: Color::WHITE,
            stroke: Color::TRANSPARENT,
            stroke_width: 0.0,
            opacity: 1.0,
            scale: 1.0,
            rotation: 0.0,
            parent: None,
        }
    }
    pub fn position(mut self, position: impl Into<Point>) -> Self {
        self.position = position.into();
        self
    }
    pub fn fill(mut self, fill: Color) -> Self {
        self.fill = fill;
        self
    }
    pub fn stroke(mut self, stroke: Color, width: f64) -> Self {
        self.stroke = stroke;
        self.stroke_width = width;
        self
    }
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }
    pub fn scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }
    pub fn rotation(mut self, rotation: f64) -> Self {
        self.rotation = rotation;
        self
    }
    /// Inherit a rectangle or circle's transform, with an independently
    /// animated local position/scale/rotation and independent style/opacity.
    pub fn relative_to(mut self, parent: impl Into<Parent>) -> Self {
        self.parent = Some(parent.into());
        self
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
        let opacity = self.opacity.clamp(0.0, 1.0) as f32;
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
        let id = CircleId(scene.graphics.circles.len());
        scene.graphics.circles.push(self);
        scene.graphics.order.push(DrawObject::Circle(id));
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
