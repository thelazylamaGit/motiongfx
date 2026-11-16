//! Styles: reusable sets of builder settings, such as [`TextStyle`].
//!
//! [`TextStyle`]: crate::TextStyle

use crate::{Object, SceneObject};

/// A style that can be applied to objects of type `T` with `.style(..)`.
pub trait ApplyStyle<T: SceneObject<M>, M: crate::Model2d> {
    /// Set the style's fields on `object`, as if its setters were called.
    fn apply(&self, object: Object<T, M>) -> Object<T, M>;
}

impl<T: SceneObject<M>, M: crate::Model2d> Object<T, M> {
    /// Apply the fields set in `style`. Later setters or styles override
    /// earlier ones, as if each setter were called here.
    ///
    /// ```
    /// use motiongfx::prelude::s;
    /// use motiongfx_2d::{signal, Scene, Text, TextStyle};
    /// let size = signal(24.0);
    /// let labels = TextStyle::new().font_size(&size);
    /// let mut scene = Scene::new();
    /// let title = scene.add(Text::new("Title").style(&labels));
    /// let mut animation = scene.compile([size.to(48.0)]);
    /// animation.sample(s(1));
    /// assert_eq!(animation.get(title).font_size, 48.0);
    /// ```
    /// Incompatible styles fail at compile time:
    /// ```compile_fail
    /// use motiongfx_2d::{Circle, TextStyle};
    /// let _: motiongfx_2d::Object<Circle> = Circle::new(10.0).style(&TextStyle::new());
    /// ```
    pub fn style(self, style: &impl ApplyStyle<T, M>) -> Self {
        style.apply(self)
    }
}

// Generates a style type whose setters mirror an object's, and applies
// them through the object's own setters.
macro_rules! partial_style {
    ($state:ident, $style:ident; $($field:ident: $value:ty),* $(,)?) => {
        /// Reusable settings to apply with `.style(..)`; only the fields you
        /// set are applied. Values can be fixed or follow signals, so
        /// animating a shared signal changes every object using it. Changing
        /// a style afterwards doesn't change objects already built with it.

        pub struct $style<M:crate::Model2d=crate::Canvas> {
            $($field: Option<crate::Input<$value,M>>,)*
        }
        impl<M:crate::Model2d> Default for $style<M> { fn default()->Self { Self { $($field:None,)* } } }
        impl<M:crate::Model2d> Clone for $style<M> { fn clone(&self)->Self { Self { $($field:self.$field.clone(),)* } } }
        impl<M:crate::Model2d> $style<M> {
            /// A style with no fields set.
            pub fn new() -> Self { Self::default() }
            $(
            #[doc = concat!("Set `", stringify!($field), "` (fixed, or a source to follow).")]
            pub fn $field(mut self, value: impl Into<crate::Input<$value,M>>) -> Self {
                self.$field = Some(value.into());
                self
            })*
        }
        impl<M:crate::Model2d> crate::ApplyStyle<$state,M> for $style<M> {
            fn apply(&self, mut object: crate::Object<$state,M>) -> crate::Object<$state,M> {
                $(if let Some(value) = &self.$field {
                    object = object.$field(value.clone());
                })*
                object
            }
        }
    };
}
pub(crate) use partial_style;
