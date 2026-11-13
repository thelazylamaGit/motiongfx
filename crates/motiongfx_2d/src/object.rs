//! Typed model builders delegate common property wiring to authoring.
use crate::{Graphics, Input, Model2d, SceneObject, World};
use motiongfx::prelude::{Path, SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use motiongfx_authoring::{ObjectDefinition, Property};
use std::ops::{Deref, DerefMut};
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
// Each generated setter accepts a fixed value or a typed source. Only dynamic
// inputs add wiring; no extra evaluator is installed for ordinary constants.
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
            $(pub fn $field(self, value: impl Into<crate::Input<$value,M>>) -> Self {
                self.property(value, motiongfx::path!($state.$field), |handle| handle.$field.into())
            })*
        }
    };
}
pub(crate) use properties;

macro_rules! stroke {
    ($state:ident) => {
        impl<M: crate::Model2d> crate::Object<$state, M> {
            pub fn stroke(
                self,
                color: impl Into<crate::Input<crate::Color, M>>,
                width: impl Into<crate::Input<f64, M>>,
            ) -> Self {
                self.property(
                    color,
                    motiongfx::path!($state.stroke),
                    |handle| handle.stroke.into(),
                )
                .property(
                    width,
                    motiongfx::path!($state.stroke_width),
                    |handle| handle.stroke_width.into(),
                )
            }
        }
    };
}
pub(crate) use stroke;

// Opt in only for shapes where stroke endpoints/corners are meaningful.
macro_rules! stroke_options {
    ($state:ident) => {
        impl<M: crate::Model2d> crate::Object<$state, M> {
            /// Stroke/dash endpoints: butt, square or round. Closed outlines
            /// have no endpoints. This does not round the shape's filled geometry.
            pub fn line_cap(
                self,
                value: impl Into<crate::Input<crate::Cap, M>>,
            ) -> Self {
                self.property(
                    value,
                    motiongfx::path!($state.line_cap),
                    |handle| handle.line_cap.into(),
                )
            }
            /// Stroke corners: miter, bevel or round. This changes the outline,
            /// not the underlying filled shape. Defaults to round.
            pub fn line_join(
                self,
                value: impl Into<crate::Input<crate::Join, M>>,
            ) -> Self {
                self.property(
                    value,
                    motiongfx::path!($state.line_join),
                    |handle| handle.line_join.into(),
                )
            }
        }
    };
}
pub(crate) use stroke_options;
