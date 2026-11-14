//! [`ObjectDefinition`]: an object's initial state plus the bindings its
//! reactive inputs need, shared by model crates' object builders.
use crate::{Input, Property, Scene, SceneObject, World};
use motiongfx::prelude::{Path, SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

/// Wires one field of a newly added object, for example by binding it to a
/// signal. Receives the scene and the object's handle.
pub type Register<T, M> = Arc<
    dyn Fn(&mut Scene<M>, <T as SceneObject<M>>::Handle)
        + Send
        + Sync,
>;
/// An object being described: its initial state, plus how to bind any
/// fields set from signals or computed values once it is added to a scene.
///
/// Model crates wrap this in their own builder type (such as
/// `motiongfx_2d::Object`) to add methods like `.fill(..)`. Dereferences to
/// the state being built.
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
    /// Start from `state`, with no fields bound.
    pub fn new(state: T) -> Self {
        Self {
            state,
            bindings: Vec::new(),
        }
    }
    /// Run `register` when the object is added, replacing any earlier hook
    /// for `field`. Each added copy of the definition runs it once.
    pub fn on_insert(
        &mut self,
        field: motiongfx::field_path::field::UntypedField,
        register: Register<T, M>,
    ) {
        self.bindings.retain(|(previous, _)| *previous != field);
        self.bindings.push((field, register));
    }
    /// Add the object with `insert`, then run its field hooks with the
    /// returned handle.
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
    /// Set the field at `path` from `value`. A fixed value becomes initial
    /// state; a signal, computed value or property binds the field when the
    /// object is added. Setting a field again replaces the earlier input.
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
