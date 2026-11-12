//! Model-neutral descriptor storage and property wiring. Model crates can wrap
//! this type to provide their own inherent shape builders and input conversions.
use crate::{Input, Property, Scene, SceneObject, World};
use motiongfx::prelude::{Path, SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

/// Construction hook used by model descriptors to wire a field after insertion.
pub type Register<T, M> = Arc<
    dyn Fn(&mut Scene<M>, <T as SceneObject<M>>::Handle)
        + Send
        + Sync,
>;
/// Initial state and construction-only property wiring. Model crates wrap this
/// to add inherent builders without duplicating binding preparation.
pub struct ObjectDefinition<T: SceneObject<M>, M> {
    state: T,
    bindings: Vec<(
        motiongfx::field_path::field::UntypedField,
        Register<T, M>,
    )>,
}
impl<T: SceneObject<M> + Clone, M> Clone for ObjectDefinition<T, M> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            bindings: self.bindings.clone(),
        }
    }
}
impl<T: SceneObject<M>, M: 'static> ObjectDefinition<T, M> {
    pub fn new(state: T) -> Self {
        Self {
            state,
            bindings: Vec::new(),
        }
    }
    /// Replace any previous hook for this field. Runs once per insertion,
    /// including insertion of a cloned descriptor, never during sampling.
    pub fn on_insert(
        &mut self,
        field: motiongfx::field_path::field::UntypedField,
        register: Register<T, M>,
    ) {
        self.bindings.retain(|(previous, _)| *previous != field);
        self.bindings.push((field, register));
    }
    pub fn insert_with(
        self,
        scene: &mut Scene<M>,
        insert: impl FnOnce(T, &mut Scene<M>) -> T::Handle,
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
    /// Fixed inputs write initial state directly; dynamic inputs register a
    /// native binding on insertion. The last setter for a field wins.
    pub fn property<I, V>(
        mut self,
        value: Input<M, V>,
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
impl<T: SceneObject<M>, M> Deref for ObjectDefinition<T, M> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.state
    }
}
impl<T: SceneObject<M>, M> DerefMut for ObjectDefinition<T, M> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.state
    }
}
impl<T: SceneObject<M>, M: 'static> SceneObject<M>
    for ObjectDefinition<T, M>
where
    T::Handle: Copy,
{
    type Handle = T::Handle;
    fn insert_into(self, scene: &mut Scene<M>) -> Self::Handle {
        self.insert_with(scene, T::insert_into)
    }
}
