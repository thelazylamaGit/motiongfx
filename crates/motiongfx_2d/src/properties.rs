//! Macros declaring each object's typed fields, builder setters and handle
//! getters from one list.

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

// Declares every animatable field of one object type, once. Each entry is
//
//     set radius: f64 => new(Circle.radius);
//
// `set` also generates a builder setter (`Circle::new(..).radius(..)`);
// `get` only the field and its handle getter, for fields set through a
// combined setter such as `stroke(color, width)` or not set directly, such
// as `position_x`. Doc comments on a `set` entry replace the setter's
// default docs. `new` fields interpolate; `discrete` ones jump.
//
// A trailing `shared` marks a field whose handle getter comes from a
// capability trait (`position` from `Transformable`, and so on), so no
// separate getter is generated for it.
//
// A leading `transform(Circle.transform);` declares the five fields of a
// `Transform` field at that path: position (with x and y), scale and
// rotation.
macro_rules! object {
    ($state:ident, $id:ident; transform($($transform:tt)+);
        $($rest:tt)*) => {
        crate::properties::object!($state, $id;
            set position: crate::Point => new($($transform)+.position) shared;
            get position_x: f64 => new($($transform)+.position.x) shared;
            get position_y: f64 => new($($transform)+.position.y) shared;
            set scale: f64 => new($($transform)+.scale) shared;
            set rotation: f64 => new($($transform)+.rotation) shared;
            $($rest)*
        );
    };
    ($state:ident, $id:ident;
        $($(#[$attr:meta])* $kind:ident $name:ident: $value:ty => $method:ident($($path:tt)+) $($shared:ident)?;)*) => {
        crate::properties::fields!(fields, $state, $id;
            $($name: $value => $method($($path)+);)*);
        $(crate::properties::setter!($kind [$(#[$attr])*] $state $name: $value => ($($path)+));)*
        $(crate::properties::getter!([$($shared)?] $state $name);)*
        impl crate::Object2d for $state {
            type Id = $id;
        }
    };
}
pub(crate) use object;

// The handle getter for one `object!` entry; nothing for a `shared` one.
macro_rules! getter {
    ([shared] $($rest:tt)*) => {};
    ([] $state:ident $name:ident) => {
        impl crate::Handle<$state> {
            #[doc = concat!("The object's `", stringify!($name), "`, to animate (`.to(..)`) or for other objects to follow.")]
            pub fn $name(self) -> fields::$name::Target {
                motiongfx_authoring::Target::new(self.id)
            }
        }
    };
}
pub(crate) use getter;

// One builder setter for a `set` entry of `object!`; nothing for `get`.
macro_rules! setter {
    (get [$($attr:tt)*] $($rest:tt)*) => {};
    (set [] $state:ident $name:ident: $value:ty => ($($path:tt)+)) => {
        crate::properties::setter!(set [
            #[doc = concat!("Set `", stringify!($name), "`: a fixed value, or a signal, computed value or property for it to follow.")]
        ] $state $name: $value => ($($path)+));
    };
    (set [$($attr:tt)+] $state:ident $name:ident: $value:ty => ($($path:tt)+)) => {
        impl<M: crate::Model2d> crate::Object<$state, M> {
            $($attr)+
            pub fn $name(
                self,
                value: impl Into<crate::Input<$value, M>>,
            ) -> Self {
                self.property(value, motiongfx::path!($($path)+), |handle| {
                    handle.$name().into()
                })
            }
        }
    };
}
pub(crate) use setter;
