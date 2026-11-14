//! Macros declaring each object's typed fields and handle methods.

// Declares a module of field markers for one object type, next to the
// object. `$method` is `new` (smooth) or `discrete`.
macro_rules! fields {
    ($module:ident, $state:ty, $id:ty; $( $name:ident: $value:ty => $method:ident($($path:tt)+); )*) => {
        #[doc = concat!("The animatable fields of [`", stringify!($state), "`].")]
        pub mod $module {
            use super::*;
            $(
            #[doc = concat!("The `", stringify!($name), "` field.")]
            pub mod $name {
                use super::*;
                #[doc = concat!("Identifies the `", stringify!($name), "` field.")]
                pub struct Marker;
                #[doc = concat!("The `", stringify!($name), "` field of one object.")]
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

// One getter per field on an object handle. `also impl<..> Type => path`
// generates the same getters on a second handle type that holds the ID at
// `self.path`, such as a line with tips.
macro_rules! handle_methods {
    ($handle:ident, $state:ty, $id:ty;
        also impl<$($generic:ident),*> $other:ty => $inner:ident.id;
        $($name:ident: $value:ty;)*) => {
        crate::properties::handle_methods!($handle, $state, $id; $($name: $value;)*);
        impl<$($generic),*> $other {
            $(
            #[doc = concat!("The line's `", stringify!($name), "`, to animate (`.to(..)`) or for other objects to follow.")]
            pub fn $name(self) -> fields::$name::Target {
                motiongfx_authoring::Target::new(self.$inner.id)
            })*
        }
    };
    ($handle:ident, $state:ty, $id:ty; $($name:ident: $value:ty;)*) => {
        impl $handle {
            $(
            #[doc = concat!("The object's `", stringify!($name), "`, to animate (`.to(..)`) or for other objects to follow.")]
            pub fn $name(self) -> fields::$name::Target {
                motiongfx_authoring::Target::new(self.id)
            })*
        }
    };
}
pub(crate) use handle_methods;

macro_rules! fill_methods {
    ($handle:ident,$state:ty,$id:ty) => {
        impl $handle {
            /// Animate the fill colour to `color`.
            #[track_caller]
            pub fn fill_to<M: crate::Model2d>(
                self,
                color: impl Into<crate::Color>,
            ) -> motiongfx_authoring::Animation<M> {
                self.fill().to(color)
            }
            /// Change the fill colour instantly.
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
            /// Move by `(x, y)` from wherever the object is when this starts.
            #[track_caller]
            pub fn shift<M: crate::Model2d>(
                self,
                x: f64,
                y: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.position().offset((x, y))
            }
            /// Move horizontally by `x`.
            #[track_caller]
            pub fn shift_x<M: crate::Model2d>(
                self,
                x: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.position_x().offset(x)
            }
            /// Move vertically by `y` (up is positive).
            #[track_caller]
            pub fn shift_y<M: crate::Model2d>(
                self,
                y: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.position_y().offset(y)
            }
            /// Scale by `factor` relative to the current scale.
            #[track_caller]
            pub fn scale_by<M: crate::Model2d>(
                self,
                factor: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.scale().multiply(factor)
            }
            /// Rotate by `angle` radians, counterclockwise.
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
