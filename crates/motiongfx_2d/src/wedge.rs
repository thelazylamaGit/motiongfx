//! Standalone triangles/chevrons and endpoint tips use the same geometry.

use crate::World;
use crate::rounded_triangle::{self, Geometry};
use crate::{
    Color, DrawObject, Object, Point, Position, SceneObject,
};
use kurbo::Affine;
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;

// One definition for both pointed shapes. A chevron is an open outline;
// a triangle closes and fills the same three vertices.
macro_rules! wedge {
    ($shape:ident, $handle:ident, $id:ident, $storage:ident, $accessor:ident, $closed:literal $(, $radius:ident => $cache:ident)?) => {
        /// A pointed shape facing +x. Position anchors its apex. `size` is
        /// axial length; `angle` is the full opening angle in radians.
        /// Chevron fill is unused because its outline is open.
        #[derive(Clone)]
        pub struct $shape {
            pub size: f64,
            pub angle: f64,
            $(/// Corner radius in local canvas units. Clamped to fit the triangle.
            pub $radius: f64,)?
            pub position: Point,
            pub rotation: f64,
            pub scale: f64,
            pub opacity: f64,
            pub fill: Color,
            pub stroke: Color,
            pub stroke_width: f64,
            pub line_cap: crate::Cap,
            pub line_join: crate::Join,
        }
        impl Default for $shape {
            fn default() -> Self {
                Self { size: 24.0, angle: std::f64::consts::FRAC_PI_3,
                    $($radius: 0.0,)?
                    position: Point::ZERO, rotation: 0.0, scale: 1.0, opacity: 1.0,
                    fill: if $closed { Color::WHITE } else { Color::TRANSPARENT },
                    stroke: if $closed { Color::TRANSPARENT } else { Color::WHITE },
                    stroke_width: if $closed { 0.0 } else { 2.0 },
                    line_cap: crate::Cap::Round, line_join: crate::Join::Round,
                }
            }
        }
        impl $shape {
            pub fn new<M:crate::Model2d>() -> Object<Self,M> { Object::new(Self::default()) }
            pub(crate) fn vertices(&self) -> [Point; 3] { vertices(self.size, self.angle) }
            pub fn transform(&self) -> Affine {
                Affine::translate((self.position.x, -self.position.y))
                    * Affine::rotate(-self.rotation) * Affine::scale(self.scale)
            }
            pub(crate) fn insert<M:crate::Model2d>(self, scene: &mut motiongfx_authoring::Scene<M>, draw: bool) -> $handle {
                let id = $id(scene.model_mut().graphics_mut().$storage.len());
                scene.model_mut().graphics_mut().$storage.push(self);

                if draw { scene.model_mut().graphics_mut().order.push(DrawObject::$shape(id)); }
                $handle {
                    id,
                    position: Position::new(id, path!($shape.position), path!($shape.position.x), path!($shape.position.y)),
                    size: Property::new(id, path!($shape.size)), angle: Property::new(id, path!($shape.angle)),
                    $($radius: Property::new(id, path!($shape.$radius)),)?
                    fill: Property::new(id, path!($shape.fill)), stroke: Property::new(id, path!($shape.stroke)),
                    stroke_width: Property::new(id, path!($shape.stroke_width)),
                    line_cap: Property::discrete(id, path!($shape.line_cap)),
                    line_join: Property::discrete(id, path!($shape.line_join)),
                    opacity: Property::new(id, path!($shape.opacity)), scale: Property::new(id, path!($shape.scale)),
                    rotation: Property::new(id, path!($shape.rotation)),
                }
            }
        }
        crate::object::properties!($shape; size: f64, angle: f64, position: Point,
            rotation: f64, scale: f64, opacity: f64, fill: Color, stroke_width: f64, $($radius: f64,)?);
        crate::object::stroke!($shape);
        crate::object::stroke_options!($shape);
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $id(pub(crate) usize);
        impl $id { pub fn index(self)->usize { self.0 } }
        #[derive(Clone, Copy)]
        pub struct $handle {
            pub(crate) id: $id,
            pub position: Position<$id, $shape>,
            pub size: Property<$id, $shape, f64>, pub angle: Property<$id, $shape, f64>,
            $(pub $radius: Property<$id, $shape, f64>,)?
            pub fill: Property<$id, $shape, Color>, pub stroke: Property<$id, $shape, Color>,
            pub line_cap: Property<$id, $shape, crate::Cap>,
            pub line_join: Property<$id, $shape, crate::Join>,
            pub stroke_width: Property<$id, $shape, f64>, pub opacity: Property<$id, $shape, f64>,
            pub scale: Property<$id, $shape, f64>, pub rotation: Property<$id, $shape, f64>,
        }
        impl $handle {pub fn id(self)->$id {self.id}}
        impl<M:crate::Model2d> SceneObject<M> for $shape {
            type Handle = $handle;
            fn insert_into(self, scene: &mut motiongfx_authoring::Scene<M>) -> $handle { self.insert(scene, true) }
        }
        impl<M:crate::Model2d> SubjectSource<$id, $shape> for World<M> {
            fn get_source(&self, id: $id) -> Option<&$shape> { self.model().graphics().$storage.get(id.0) }
            fn apply_source<R>(&mut self, id: $id, f: impl FnOnce(&mut $shape) -> R) -> Option<R> {
                self.model_mut().graphics_mut().$storage.get_mut(id.0).map(f)
            }
        }

    };
}
wedge!(
    Triangle,
    TriangleHandle,
    TriangleId,
    triangles,
    triangle,
    true,
    radius => triangle_geometry
);
wedge!(Chevron, ChevronHandle, ChevronId, chevrons, chevron, false);

impl Triangle {
    pub fn geometry(&self) -> Geometry {
        rounded_triangle::rounded(self.vertices(), self.radius)
    }
}
impl Chevron {
    pub fn geometry(&self) -> Geometry {
        Geometry::sharp(self.vertices(), false)
    }
    pub fn extent(&self) -> f64 {
        self.geometry().extent(
            self.position,
            self.rotation,
            self.scale,
        )
    }
}

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
