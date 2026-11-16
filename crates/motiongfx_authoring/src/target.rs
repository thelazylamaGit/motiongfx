//! [`Target`]: a compact [`Property`] for fields known at compile time.
use crate::{
    Animation, BindingPlan, BindingSource, Computed, Property,
    Source, World,
};
use motiongfx::prelude::*;
use motiongfx::subject::SubjectId;
use std::{
    marker::PhantomData,
    ops::{Add, Mul},
    time::Duration,
};

/// A field known at compile time, such as a circle's radius. Implemented
/// by empty marker types that model crates declare for each field.
pub trait Field<S, T>: Send + Sync + 'static {
    /// The field of object `id`.
    fn property<I: Copy>(id: I) -> Property<I, S, T>;
}

/// One field of one object, like [`Property`], but storing only the
/// object's ID: the field is part of the type (`F`). Object handles return
/// these, e.g. `circle.radius()`.
#[must_use = "a field does nothing until animated (`.to(..)`) or followed"]
pub struct Target<I, S, T, F> {
    id: I,
    // `fn() -> ..` names the types without owning values of them, so a
    // `Target` is `Copy`, `Send` and `Sync` whatever they are.
    #[expect(
        clippy::type_complexity,
        reason = "a marker type; naming it would add nothing"
    )]
    marker: PhantomData<fn() -> (S, T, F)>,
}
/// Shows the object ID and the field's type name.
impl<I: std::fmt::Debug, S, T, F> std::fmt::Debug
    for Target<I, S, T, F>
{
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Target")
            .field("id", &self.id)
            .field("field", &std::any::type_name::<F>())
            .finish()
    }
}
impl<I: Copy, S, T, F> Copy for Target<I, S, T, F> {}
impl<I: Copy, S, T, F> Clone for Target<I, S, T, F> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<I: Copy, S, T, F: Field<S, T>> Target<I, S, T, F> {
    /// The field `F` of object `id`.
    pub const fn new(id: I) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }
    /// The same field as a [`Property`].
    pub fn property(self) -> Property<I, S, T> {
        F::property(self.id)
    }
    /// A value calculated from this field, for other objects to follow.
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
/// Animating the field; the same methods as [`Property`]'s.
impl<I, S, T, F> Target<I, S, T, F>
where
    I: SubjectId,
    S: Clone + ThreadSafe,
    T: Clone + ThreadSafe,
    F: Field<S, T>,
{
    /// Animate to a target calculated from the value at the change's
    /// start. Runs once while compiling, never per frame.
    #[track_caller]
    pub fn to_with<M: 'static>(
        self,
        target: impl Fn(&T) -> T + Clone + ThreadSafe,
    ) -> Animation<M>
    where
        World<M>: SubjectSource<I, S>,
    {
        crate::animation::change(self, target)
    }

    /// Animate to `value`.
    #[track_caller]
    pub fn to<M: 'static>(self, value: impl Into<T>) -> Animation<M>
    where
        World<M>: SubjectSource<I, S>,
    {
        let value = value.into();
        self.to_with(move |_| value.clone())
    }

    /// Replace the value instantly at the point where this is scheduled.
    #[track_caller]
    pub fn set<M: 'static>(self, value: impl Into<T>) -> Animation<M>
    where
        World<M>: SubjectSource<I, S>,
    {
        self.to(value).over(Duration::ZERO)
    }

    /// Animate by adding `delta` to the value at the change's start.
    #[track_caller]
    pub fn offset<M: 'static, D>(self, delta: D) -> Animation<M>
    where
        T: Add<D, Output = T>,
        D: Copy + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        self.to_with(move |previous: &T| previous.clone() + delta)
    }

    /// Animate by multiplying the value at the change's start by `factor`.
    #[track_caller]
    pub fn multiply<M: 'static, D>(self, factor: D) -> Animation<M>
    where
        T: Mul<D, Output = T>,
        D: Copy + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        self.to_with(move |previous: &T| previous.clone() * factor)
    }

    /// Animate a point or vector to `(x, y)`.
    #[track_caller]
    pub fn to_xy<M: 'static>(self, x: f64, y: f64) -> Animation<M>
    where
        T: From<(f64, f64)>,
        World<M>: SubjectSource<I, S>,
    {
        self.to(T::from((x, y)))
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
