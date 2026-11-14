//! Macros declaring each object's typed fields and handle methods.

// Declare field identity beside the owning object. Markers contain no data;
// only a Target's subject ID occupies storage. General Property expansion is
// shared with the ordinary animation/binding implementation.
macro_rules! fields {
    ($module:ident, $state:ty, $id:ty; $( $name:ident: $value:ty => $method:ident($($path:tt)+); )*) => {
        pub mod $module {
            use super::*;
            $(pub mod $name {
                use super::*;
                pub struct Marker;
                pub type Target = motiongfx_authoring::Target<$id,$state,$value,Marker>;
                impl motiongfx_authoring::Field<$state,$value> for Marker {
                    fn property<I:Copy>(id:I)->motiongfx_authoring::Property<I,$state,$value> {
                        motiongfx_authoring::Property::$method(id,motiongfx::path!($($path)+))
                    }
                }
            })*
        }
    }
}
pub(crate) use fields;

// Getter methods materialize only one typed target; each object handle owns one ID.
macro_rules! handle_methods {
    ($handle:ident, $state:ty, $id:ty; $($name:ident: $value:ty;)*) => {
        impl $handle {
            $(pub fn $name(self) -> fields::$name::Target {
                motiongfx_authoring::Target::new(self.id)
            })*
        }
    }
}
pub(crate) use handle_methods;

macro_rules! fill_methods {
    ($handle:ident,$state:ty,$id:ty) => {
        impl $handle {
            /// Animate fill using the scene defaults unless timed.
            #[track_caller]
            pub fn fill_to<M: crate::Model2d>(
                self,
                color: impl Into<crate::Color>,
            ) -> motiongfx_authoring::Animation<M> {
                self.fill().to(color)
            }
            /// Replace fill at the scheduled time without consuming duration.
            #[track_caller]
            pub fn set_fill<M: crate::Model2d>(
                self,
                color: impl Into<crate::Color>,
            ) -> motiongfx_authoring::Animation<M> {
                self.fill().set(color)
            }
        }
    };
}
pub(crate) use fill_methods;

macro_rules! transform_methods {
    ($handle:ident,$state:ty,$id:ty) => {
        impl $handle {
            /// Offset position from its action-start value.
            #[track_caller]
            pub fn shift<M: crate::Model2d>(
                self,
                x: f64,
                y: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.position().offset((x, y))
            }
            #[track_caller]
            pub fn shift_x<M: crate::Model2d>(
                self,
                x: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.position_x().offset(x)
            }
            #[track_caller]
            pub fn shift_y<M: crate::Model2d>(
                self,
                y: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.position_y().offset(y)
            }
            /// Multiply scale from its action-start value.
            #[track_caller]
            pub fn scale_by<M: crate::Model2d>(
                self,
                factor: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.scale().multiply(factor)
            }
            /// Offset rotation in counterclockwise radians.
            #[track_caller]
            pub fn rotate<M: crate::Model2d>(
                self,
                angle: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.rotation().offset(angle)
            }
        }
    };
}
pub(crate) use transform_methods;
