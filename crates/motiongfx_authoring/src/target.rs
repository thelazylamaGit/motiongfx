//! Compact built-in field targets. Metadata is materialized only when needed.
use crate::{BindingPlan, BindingSource, Computed, Property, Source};
use motiongfx::prelude::*;
use motiongfx::subject::SubjectId;
use std::{
    marker::PhantomData,
    ops::{Add, Mul},
};

/// A statically known field. Implementations are zero-sized marker types;
/// the general `Property` remains available for runtime-selected paths.
pub trait Field<S, T>: Send + Sync + 'static {
    fn property<I: Copy>(id: I) -> Property<I, S, T>;
}

/// Object identity with a field selected by its type, without stored accessors.
pub struct Target<I, S, T, F> {
    id: I,
    // Field/subject/value types identify metadata, not owned object storage.
    #[allow(clippy::type_complexity)]
    marker: PhantomData<fn() -> (S, T, F)>,
}
impl<I: Copy, S, T, F> Copy for Target<I, S, T, F> {}
impl<I: Copy, S, T, F> Clone for Target<I, S, T, F> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<I: Copy, S, T, F: Field<S, T>> Target<I, S, T, F> {
    pub const fn new(id: I) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }
    /// Expand into the existing general property representation without allocation.
    pub fn property(self) -> Property<I, S, T> {
        F::property(self.id)
    }
    pub fn map_target<A: Fn(&T) -> T + ThreadSafe>(
        self,
        target: A,
    ) -> crate::Change<I, S, T, A> {
        self.property().map_target(target)
    }
    pub fn to(
        self,
        value: impl Into<T>,
    ) -> crate::Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + ThreadSafe,
    {
        self.property().to(value)
    }
    pub fn set(
        self,
        value: impl Into<T>,
    ) -> crate::Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + ThreadSafe,
    {
        self.property().set(value)
    }
    /// Coordinate convenience for point/vector values convertible from an xy pair.
    pub fn to_xy(
        self,
        x: f64,
        y: f64,
    ) -> crate::Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: From<(f64, f64)> + Clone + ThreadSafe,
    {
        self.to(T::from((x, y)))
    }
    pub fn offset<D: Copy + ThreadSafe>(
        self,
        delta: D,
    ) -> crate::Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + Add<D, Output = T>,
    {
        self.property().offset(delta)
    }
    pub fn multiply<D: Copy + ThreadSafe>(
        self,
        factor: D,
    ) -> crate::Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + Mul<D, Output = T>,
    {
        self.property().multiply(factor)
    }
    pub fn spring_to<M>(
        self,
        value: impl Into<T>,
        spring: impl Into<crate::Spring>,
    ) -> crate::SpringChange<I, S, T, M>
    where
        T: crate::SpringValue<M> + Clone + ThreadSafe,
    {
        self.property().spring_to(value, spring)
    }
    pub fn map<W: 'static, U, A>(
        &self,
        calculate: A,
    ) -> Computed<W, U>
    where
        Self: Source<W>,
        U: Clone + PartialEq + ThreadSafe,
        A: Fn(<Self as Source<W>>::Value) -> U + ThreadSafe,
    {
        Computed::new(self, calculate)
    }
}
impl<I: Copy, S, T, F: Field<S, T>> From<Target<I, S, T, F>>
    for Property<I, S, T>
{
    fn from(target: Target<I, S, T, F>) -> Self {
        target.property()
    }
}
impl<W, I, S, T, F> Source<W> for Target<I, S, T, F>
where
    I: SubjectId,
    S: 'static,
    T: Clone + PartialEq + ThreadSafe,
    F: Field<S, T>,
    W: SubjectSource<I, S>,
{
    type Value = T;
    type Prepared = Property<I, S, T>;
    type Owned = Self;
    fn owned(&self) -> Self {
        *self
    }
    fn register(
        &self,
        _: &mut W,
        _: &mut BindingPlan<W>,
    ) -> Self::Prepared {
        self.property()
    }
}
impl<W, I, S, T, F> BindingSource<W> for Target<I, S, T, F>
where
    I: SubjectId,
    S: 'static,
    T: Clone + PartialEq + ThreadSafe,
    F: Field<S, T>,
    W: SubjectSource<I, S>,
{
    type Value = T;
    fn read(
        &self,
        world: &W,
        values: &crate::bindings::DerivedValues,
    ) -> Option<T> {
        self.property().read(world, values)
    }
    fn dependencies(
        &self,
    ) -> Vec<crate::bindings::BindingDependency> {
        <Property<I, S, T> as BindingSource<W>>::dependencies(
            &self.property(),
        )
    }
    fn validate(&self, world: &W) -> Result<(), crate::BindingError> {
        self.property().validate(world)
    }
}
