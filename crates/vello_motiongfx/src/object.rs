//! Construction-only descriptors. Reactive wiring is removed from the object
//! when it is inserted: sampled state keeps ordinary numeric/style fields.

use crate::{Graphics, Input, Scene, SceneObject};
use motiongfx::prelude::{Path, SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use motiongfx_authoring::Property;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

type Register<T> =
    Arc<dyn Fn(&mut Scene, <T as SceneObject>::Handle) + Send + Sync>;
type Field = motiongfx::field_path::field::UntypedField;

/// Initial object state and optional source-driven properties. Cloning a
/// descriptor creates another object on insertion, sharing its input sources.
pub struct Object<T: SceneObject> {
    state: T,
    bindings: Vec<(Field, Register<T>)>,
}
impl<T: SceneObject + Clone> Clone for Object<T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            bindings: self.bindings.clone(),
        }
    }
}
impl<T: SceneObject> Object<T> {
    pub(crate) fn insert_with(
        self,
        scene: &mut Scene,
        insert: impl FnOnce(T, &mut Scene) -> T::Handle,
    ) -> T::Handle
    where
        T::Handle: Copy,
    {
        let handle = insert(self.state, scene);
        for (_, register) in self.bindings {
            register(scene, handle);
        }
        handle
    }
    pub(crate) fn new(state: T) -> Self {
        Self {
            state,
            bindings: Vec::new(),
        }
    }

    pub(crate) fn on_insert(
        &mut self,
        field: Field,
        register: Register<T>,
    ) {
        self.bindings.retain(|(previous, _)| *previous != field);
        self.bindings.push((field, register));
    }

    pub(crate) fn property<I, V>(
        mut self,
        value: impl Into<Input<V>>,
        path: Path<T, V>,
        target: impl Fn(T::Handle) -> Property<I, T, V> + ThreadSafe,
    ) -> Self
    where
        T: 'static,
        T::Handle: Copy,
        I: SubjectId,
        V: Clone + PartialEq + ThreadSafe,
        Graphics: SubjectSource<I, T>,
    {
        let value = value.into();
        self.bindings.retain(|(previous, _)| {
            *previous != path.field.untyped()
        });
        if let Input::Fixed(value) = value {
            *path.lens.get_mut(&mut self.state) = value;
        } else {
            self.on_insert(
                path.field.untyped(),
                Arc::new(move |scene, handle| {
                    scene.bind(
                        target(handle),
                        value.clone(),
                        std::convert::identity,
                    );
                }),
            );
        }
        self
    }
}

impl<T: SceneObject> Deref for Object<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.state
    }
}
impl<T: SceneObject> DerefMut for Object<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.state
    }
}
impl<T: SceneObject> SceneObject for Object<T>
where
    T::Handle: Copy,
{
    type Handle = T::Handle;
    fn insert_into(self, scene: &mut Scene) -> Self::Handle {
        self.insert_with(scene, T::insert_into)
    }
}

// Each generated setter accepts a fixed value or a typed source. Only dynamic
// inputs add wiring; no extra evaluator is installed for ordinary constants.
macro_rules! properties {
    ($state:ident; $($field:ident: $value:ty),* $(,)?) => {
        impl crate::Object<$state> {
            $(pub fn $field(self, value: impl Into<crate::Input<$value>>) -> Self {
                self.property(value, motiongfx::path!($state.$field), |handle| handle.$field.into())
            })*
        }
    };
}
pub(crate) use properties;

macro_rules! stroke {
    ($state:ident) => {
        impl crate::Object<$state> {
            pub fn stroke(
                self,
                color: impl Into<crate::Input<crate::Color>>,
                width: impl Into<crate::Input<f64>>,
            ) -> Self {
                self.property(
                    color,
                    motiongfx::path!($state.stroke),
                    |handle| handle.stroke,
                )
                .property(
                    width,
                    motiongfx::path!($state.stroke_width),
                    |handle| handle.stroke_width,
                )
            }
        }
    };
}
pub(crate) use stroke;
