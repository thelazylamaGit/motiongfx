//! Standalone triangles/chevrons and endpoint tips use the same geometry.

use crate::{
    Color, DrawObject, Graphics, Object, Point, Position, Scene,
    SceneObject,
};
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
use vello::kurbo::{Affine, PathEl, Stroke};
use vello::peniko::Fill;

// One definition for both pointed shapes. A chevron is an open outline;
// a triangle closes and fills the same three vertices.
macro_rules! wedge {
    ($shape:ident, $handle:ident, $id:ident, $storage:ident, $accessor:ident, $closed:literal) => {
        /// A pointed shape facing +x. Position anchors its apex. `size` is
        /// axial length; `angle` is the full opening angle in radians.
        /// Chevron fill is unused because its outline is open.
        #[derive(Clone)]
        pub struct $shape {
            pub size: f64,
            pub angle: f64,
            pub position: Point,
            pub rotation: f64,
            pub scale: f64,
            pub opacity: f64,
            pub fill: Color,
            pub stroke: Color,
            pub stroke_width: f64,
        }
        impl Default for $shape {
            fn default() -> Self {
                Self { size: 24.0, angle: std::f64::consts::FRAC_PI_3,
                    position: Point::ZERO, rotation: 0.0, scale: 1.0, opacity: 1.0,
                    fill: if $closed { Color::WHITE } else { Color::TRANSPARENT },
                    stroke: if $closed { Color::TRANSPARENT } else { Color::WHITE },
                    stroke_width: if $closed { 0.0 } else { 2.0 },
                }
            }
        }
        impl $shape {
            pub fn new() -> Object<Self> { Object::new(Self::default()) }
            pub(crate) fn vertices(&self) -> [Point; 3] { vertices(self.size, self.angle) }
            pub(crate) fn transform(&self) -> Affine {
                Affine::translate((self.position.x, -self.position.y))
                    * Affine::rotate(-self.rotation) * Affine::scale(self.scale)
            }
            pub(crate) fn extent(&self) -> f64 {
                let (sin, cos) = self.rotation.sin_cos();
                self.vertices().iter().map(|p| {
                    let x = self.position.x + self.scale * (p.x * cos - p.y * sin);
                    (-x).max(0.0)
                }).fold(0.0, f64::max)
            }
            pub(crate) fn render(&self, scene: &mut vello::Scene, parent: Affine, inherited: f32) {
                let opacity = self.opacity.clamp(0.0, 1.0) as f32 * inherited;
                let transform = parent * self.transform();
                if self.size <= 0.0 || opacity <= 0.0 || transform.determinant() == 0.0 { return; }
                let [tip, a, b] = self.vertices().map(|p| Point::new(p.x, -p.y));
                let geometry = [PathEl::MoveTo(a), PathEl::LineTo(tip), PathEl::LineTo(b), PathEl::ClosePath];
                if $closed && self.fill.components[3] > 0.0 {
                    scene.fill(Fill::NonZero, transform, self.fill.with_alpha(self.fill.components[3] * opacity), None, &geometry);
                }
                if self.stroke_width > 0.0 && self.stroke.components[3] > 0.0 {
                    let outline = if $closed { &geometry[..] } else { &geometry[..3] };
                    scene.stroke(&Stroke::new(self.stroke_width), transform,
                        self.stroke.with_alpha(self.stroke.components[3] * opacity), None, &outline);
                }
            }
            pub(crate) fn insert(self, scene: &mut Scene, draw: bool) -> $handle {
                let id = $id(scene.graphics.$storage.len());
                scene.graphics.$storage.push(self);
                if draw { scene.graphics.order.push(DrawObject::$shape(id)); }
                $handle {
                    id,
                    position: Position::new(id, path!($shape.position), path!($shape.position.x), path!($shape.position.y)),
                    size: Property::new(id, path!($shape.size)), angle: Property::new(id, path!($shape.angle)),
                    fill: Property::new(id, path!($shape.fill)), stroke: Property::new(id, path!($shape.stroke)),
                    stroke_width: Property::new(id, path!($shape.stroke_width)),
                    opacity: Property::new(id, path!($shape.opacity)), scale: Property::new(id, path!($shape.scale)),
                    rotation: Property::new(id, path!($shape.rotation)),
                }
            }
        }
        crate::object::properties!($shape; size: f64, angle: f64, position: Point,
            rotation: f64, scale: f64, opacity: f64, fill: Color, stroke_width: f64,);
        crate::object::stroke!($shape);
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $id(pub(crate) usize);
        #[derive(Clone, Copy)]
        pub struct $handle {
            pub(crate) id: $id,
            pub position: Position<$id, $shape>,
            pub size: Property<$id, $shape, f64>, pub angle: Property<$id, $shape, f64>,
            pub fill: Property<$id, $shape, Color>, pub stroke: Property<$id, $shape, Color>,
            pub stroke_width: Property<$id, $shape, f64>, pub opacity: Property<$id, $shape, f64>,
            pub scale: Property<$id, $shape, f64>, pub rotation: Property<$id, $shape, f64>,
        }
        impl SceneObject for $shape {
            type Handle = $handle;
            fn insert_into(self, scene: &mut Scene) -> $handle { self.insert(scene, true) }
        }
        impl SubjectSource<$id, $shape> for Graphics {
            fn get_source(&self, id: $id) -> Option<&$shape> { self.$storage.get(id.0) }
            fn apply_source<R>(&mut self, id: $id, f: impl FnOnce(&mut $shape) -> R) -> Option<R> {
                self.$storage.get_mut(id.0).map(f)
            }
        }
        impl crate::Animation {
            pub fn $accessor(&self, handle: $handle) -> &$shape { &self.graphics.$storage[handle.id.0] }
        }
    };
}
wedge!(
    Triangle,
    TriangleHandle,
    TriangleId,
    triangles,
    triangle,
    true
);
wedge!(Chevron, ChevronHandle, ChevronId, chevrons, chevron, false);

fn vertices(size: f64, angle: f64) -> [Point; 3] {
    assert!(
        size.is_finite() && size >= 0.0,
        "shape size must be finite and nonnegative"
    );
    assert!(
        angle.is_finite()
            && angle > 0.0
            && angle < std::f64::consts::PI,
        "opening angle must be between zero and pi radians"
    );
    let half_width = size * (angle / 2.0).tan();
    [
        Point::ZERO,
        Point::new(-size, half_width),
        Point::new(-size, -half_width),
    ]
}
