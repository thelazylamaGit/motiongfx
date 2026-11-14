//! Triangles and chevrons: the same pointed shape, filled or open. Both work
//! on their own or as line tips.

use crate::Point;

// One definition for both pointed shapes. A chevron is an open outline;
// a triangle closes and fills the same three vertices.
macro_rules! wedge {
    ($shape:ident, $handle:ident, $id:ident, $storage:ident, $accessor:ident, $closed:literal $(, $radius:ident => $cache:ident)?) => {
        /// A pointed shape facing +x, with its apex at `position`. Create
        /// one with `new()`. A chevron is an open outline, so its fill is
        /// unused.
        #[derive(Clone)]
        pub struct $shape {
            /// Length from the apex to the base, in canvas units.
            pub size: f64,
            /// Full opening angle at the apex, in radians.
            pub angle: f64,
            $(/// Corner radius in canvas units, reduced if it doesn't fit.
            pub $radius: f64,)?
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
            /// How open outline ends are drawn.
            pub line_cap: crate::Cap,
            /// How outline corners are drawn.
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
            /// A shape 24 units long with a 60° opening; set other fields with
            /// the builder methods.
            pub fn new<M:crate::Model2d>() -> Object<Self,M> { Object::new(Self::default()) }
            pub(crate) fn vertices(&self) -> [Point; 3] { crate::wedge::vertices(self.size, self.angle) }
            /// Where the shape's local coordinates sit on the canvas.
            pub fn transform(&self) -> Affine {
                Affine::translate((self.position.x, -self.position.y))
                    * Affine::rotate(-self.rotation) * Affine::scale(self.scale)
            }
            /// Add the shape and draw it in insertion order.
            pub(crate) fn insert<M:crate::Model2d>(self, scene: &mut motiongfx_authoring::Scene<M>) -> $handle {
                let handle = self.insert_tip(scene);
                scene.model_mut().graphics_mut().order.push(DrawObject::$shape(handle.id));
                handle
            }
            /// Add the shape without drawing it; its line draws it as a tip.
            pub(crate) fn insert_tip<M:crate::Model2d>(self, scene: &mut motiongfx_authoring::Scene<M>) -> $handle {
                let storage = &mut scene.model_mut().graphics_mut().$storage;
                storage.push(self);
                $handle { id: $id(storage.len() - 1) }
            }
        }
        crate::object::properties!($shape; size: f64, angle: f64, position: Point,
            rotation: f64, scale: f64, opacity: f64, fill: Color, stroke_width: f64, $($radius: f64,)?);
        crate::object::stroke!($shape);
        crate::object::stroke_options!($shape);
        #[doc = concat!("Identifies one [`", stringify!($shape), "`] in its scene.")]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $id(pub(crate) usize);
        impl $id {
            /// Its position in the scene's storage for this shape type.
            pub fn index(self)->usize { self.0 }
        }
        #[doc = concat!("A [`", stringify!($shape), "`] in a scene, returned by `scene.add`. Its methods return fields to animate.")]
        #[derive(Clone, Copy, Debug)]
        pub struct $handle { pub(crate) id: $id }
        impl $handle {
            /// The shape's ID.
            pub fn id(self)->$id {self.id}
        }
        impl<M:crate::Model2d> SceneObject<M> for $shape {
            type Handle = $handle;
            fn insert_into(self, scene: &mut motiongfx_authoring::Scene<M>) -> $handle { self.insert(scene) }
        }
        impl<M:crate::Model2d> SubjectSource<$id, $shape> for World<M> {
            fn get_source(&self, id: $id) -> Option<&$shape> { self.model().graphics().$storage.get(id.0) }
            fn apply_source<R>(&mut self, id: $id, f: impl FnOnce(&mut $shape) -> R) -> Option<R> {
                self.model_mut().graphics_mut().$storage.get_mut(id.0).map(f)
            }
        }

    };
}
pub(crate) use wedge;

pub(crate) fn vertices(size: f64, angle: f64) -> [Point; 3] {
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
