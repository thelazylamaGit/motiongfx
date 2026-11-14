//! Typed model builders delegate common property wiring to authoring.
use crate::{Graphics, Input, Model2d, SceneObject, World};
use motiongfx::prelude::{Path, SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use motiongfx_authoring::{ObjectDefinition, Property};
use std::ops::{Deref, DerefMut};
/// An object description being built, such as `Circle::new(10.0).fill(..)`.
#[must_use = "an object is only drawn after it is passed to `scene.add(..)`"]
pub struct Object<T: SceneObject<M>, M: Model2d = Graphics>(
    ObjectDefinition<T, M>,
);
impl<T: SceneObject<M> + Clone, M: Model2d> Clone for Object<T, M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: SceneObject<M>, M: Model2d> Object<T, M> {
    pub(crate) fn new(state: T) -> Self {
        Self(ObjectDefinition::new(state))
    }
    pub(crate) fn insert_with(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
        insert: impl FnOnce(
            T,
            &mut motiongfx_authoring::Scene<M>,
        ) -> T::Handle,
    ) -> T::Handle
    where
        T::Handle: Copy,
    {
        self.0.insert_with(scene, insert)
    }
    pub(crate) fn on_insert(
        &mut self,
        field: motiongfx::field_path::field::UntypedField,
        register: motiongfx_authoring::Register<T, M>,
    ) {
        self.0.on_insert(field, register);
    }
    pub(crate) fn property<I, V>(
        self,
        value: impl Into<Input<V, M>>,
        path: Path<T, V>,
        target: impl Fn(T::Handle) -> Property<I, T, V> + ThreadSafe,
    ) -> Self
    where
        T: 'static,
        T::Handle: Copy,
        I: SubjectId,
        V: Clone + PartialEq + ThreadSafe,
        World<M>: SubjectSource<I, T>,
    {
        Self(self.0.property(value.into().0, path, target))
    }
}
impl<T: SceneObject<M>, M: Model2d> Deref for Object<T, M> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T: SceneObject<M>, M: Model2d> DerefMut for Object<T, M> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
impl<T: SceneObject<M>, M: Model2d> SceneObject<M> for Object<T, M>
where
    T::Handle: Copy,
{
    type Handle = T::Handle;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> T::Handle {
        self.0.insert_into(scene)
    }
}
// One builder setter per field. Each takes a fixed value or a source.
macro_rules! properties {
    ($state:ident; $($field:ident: $value:ty),* $(,)?;
        style $style:ident { $($styled:ident: $style_value:ty),* $(,)? }) => {
        crate::object::properties!($state;
            $($field: $value,)* $($styled: $style_value,)*
        );
        crate::style::partial_style!($state, $style; $($styled: $style_value,)*);
    };
    ($state:ident; $($field:ident: $value:ty),* $(,)?) => {
        impl<M:crate::Model2d> crate::Object<$state,M> {
            $(
            #[doc = concat!("Set `", stringify!($field), "`: a fixed value, or a signal, computed value or property for it to follow.")]
            pub fn $field(self, value: impl Into<crate::Input<$value,M>>) -> Self {
                self.property(value, motiongfx::path!($state.$field), |handle| handle.$field().into())
            })*
        }
    };
}
pub(crate) use properties;

macro_rules! stroke {
    ($state:ident) => {
        impl<M: crate::Model2d> crate::Object<$state, M> {
            /// Set the outline colour and width (fixed values or sources).
            pub fn stroke(
                self,
                color: impl Into<crate::Input<crate::Color, M>>,
                width: impl Into<crate::Input<f64, M>>,
            ) -> Self {
                self.property(
                    color,
                    motiongfx::path!($state.stroke),
                    |handle| handle.stroke().into(),
                )
                .property(
                    width,
                    motiongfx::path!($state.stroke_width),
                    |handle| handle.stroke_width().into(),
                )
            }
        }
    };
}
pub(crate) use stroke;

// Cap and join setters, for shapes whose strokes have ends or corners.
macro_rules! stroke_options {
    ($state:ident) => {
        impl<M: crate::Model2d> crate::Object<$state, M> {
            /// How open stroke ends and dash ends are drawn: butt, square or
            /// round (the default). Closed outlines have no ends.
            pub fn line_cap(
                self,
                value: impl Into<crate::Input<crate::Cap, M>>,
            ) -> Self {
                self.property(
                    value,
                    motiongfx::path!($state.line_cap),
                    |handle| handle.line_cap().into(),
                )
            }
            /// How stroke corners are drawn: miter, bevel or round (the
            /// default). The fill keeps its sharp corners.
            pub fn line_join(
                self,
                value: impl Into<crate::Input<crate::Join, M>>,
            ) -> Self {
                self.property(
                    value,
                    motiongfx::path!($state.line_join),
                    |handle| handle.line_join().into(),
                )
            }
        }
    };
}
pub(crate) use stroke_options;
