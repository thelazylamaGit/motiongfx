//! Standalone triangles/chevrons and endpoint tips use the same geometry.

use crate::Point;

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
            pub(crate) fn vertices(&self) -> [Point; 3] { crate::wedge::vertices(self.size, self.angle) }
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
                    position: Position::from_targets(motiongfx_authoring::Target::new(id),motiongfx_authoring::Target::new(id),motiongfx_authoring::Target::new(id)),
                    size: motiongfx_authoring::Target::new(id), angle: motiongfx_authoring::Target::new(id),
                    $($radius: motiongfx_authoring::Target::new(id),)?
                    fill: motiongfx_authoring::Target::new(id), stroke: motiongfx_authoring::Target::new(id),
                    stroke_width: motiongfx_authoring::Target::new(id),
                    line_cap: motiongfx_authoring::Target::new(id),
                    line_join: motiongfx_authoring::Target::new(id),
                    opacity: motiongfx_authoring::Target::new(id), scale: motiongfx_authoring::Target::new(id),
                    rotation: motiongfx_authoring::Target::new(id),
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
            pub position: Position<$id, $shape, fields::position::Target, fields::position_x::Target, fields::position_y::Target>,
            pub size: fields::size::Target, pub angle: fields::angle::Target,
            $(pub $radius: fields::$radius::Target,)?
            pub fill: fields::fill::Target, pub stroke: fields::stroke::Target,
            pub line_cap: fields::line_cap::Target,
            pub line_join: fields::line_join::Target,
            pub stroke_width: fields::stroke_width::Target, pub opacity: fields::opacity::Target,
            pub scale: fields::scale::Target, pub rotation: fields::rotation::Target,
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
