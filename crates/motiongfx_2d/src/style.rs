//! Typed partial styles, applied only during object construction.

use crate::{Object, SceneObject};

/// A reusable property set applicable to one concrete object type.
/// Implementations apply through ordinary object setters so reactive wiring
/// and last-setter-wins semantics stay identical to explicit authoring.
pub trait ApplyStyle<T: SceneObject<M>, M: crate::Model2d> {
    fn apply(&self, object: Object<T, M>) -> Object<T, M>;
}

impl<T: SceneObject<M>, M: crate::Model2d> Object<T, M> {
    /// Apply only the properties configured in this style. Later setters or
    /// styles override earlier ones. The object retains its own inputs; the
    /// style is not stored or evaluated in playback.
    ///
    /// ```
    /// use motiongfx::prelude::s;
    /// use motiongfx_2d::{Animation2dExt,signal, Scene, Text, TextStyle};
    /// let size = signal(24.0);
    /// let labels = TextStyle::new().font_size(&size);
    /// let mut scene = Scene::new();
    /// let title = scene.add(Text::new("Title").style(&labels));
    /// let mut animation = scene.animate(|sequence| {
    ///     sequence.play(size.to(48.0));
    /// });
    /// animation.sample(s(1));
    /// assert_eq!(animation.text(title).font_size, 48.0);
    /// ```
    /// Incompatible styles fail at compile time:
    /// ```compile_fail
    /// use motiongfx_2d::{Animation2dExt,Circle, TextStyle};
    /// let _: motiongfx_2d::Object<Circle> = Circle::new(10.0).style(&TextStyle::new());
    /// ```
    pub fn style(self, style: &impl ApplyStyle<T, M>) -> Self {
        style.apply(self)
    }
}

// Called by the property declaration macro: style setters and object setters
// share the same names/types. Applying a style reuses those object setters.
macro_rules! partial_style {
    ($state:ident, $style:ident; $($field:ident: $value:ty),* $(,)?) => {
        /// A reusable, construction-time partial style. Unspecified properties
        /// remain unchanged. Inputs may be constants, signals, or computations.
        /// Clone to derive another preset; changing a preset does not mutate
        /// existing objects. Animate shared inputs to coordinate those objects.

        pub struct $style<M:crate::Model2d=crate::Graphics> {
            $($field: Option<crate::Input<$value,M>>,)*
        }
        impl<M:crate::Model2d> Default for $style<M> { fn default()->Self { Self { $($field:None,)* } } }
        impl<M:crate::Model2d> Clone for $style<M> { fn clone(&self)->Self { Self { $($field:self.$field.clone(),)* } } }
        impl<M:crate::Model2d> $style<M> {
            pub fn new() -> Self { Self::default() }
            $(pub fn $field(mut self, value: impl Into<crate::Input<$value,M>>) -> Self {
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
