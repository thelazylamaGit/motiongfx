//! Model-specific conversions over shared, prepared reactive inputs.
use crate::{
    Computed, Graphics, Model2d, Point, Signal, Size, Vec2, World,
};
use motiongfx::prelude::{SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use motiongfx_authoring::{BindingPlan, Property, Source};
use std::sync::Arc;
pub struct Input<T, M = Graphics>(
    pub(crate) motiongfx_authoring::Input<M, T>,
);
impl<T: Clone, M> Clone for Input<T, M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T, M> Input<T, M> {
    fn fixed(value: T) -> Self {
        Self(motiongfx_authoring::Input::Fixed(value))
    }
    pub fn as_fixed(&self) -> Option<&T> {
        self.0.as_fixed()
    }
}
impl<T: Clone + PartialEq + ThreadSafe, M: Model2d> Input<T, M> {
    fn mapped<S, F>(source: S, convert: F) -> Self
    where
        S: Source<World<M>>,
        F: Fn(S::Value) -> T + ThreadSafe,
    {
        Self(motiongfx_authoring::Input::mapped(source, convert))
    }
}
impl<T, M> From<T> for Input<T, M> {
    fn from(value: T) -> Self {
        Self::fixed(value)
    }
}
impl<T, M> From<Signal<T>> for Input<T, M> {
    fn from(value: Signal<T>) -> Self {
        Self(motiongfx_authoring::Input::Signal(value))
    }
}
impl<T: Clone, M> From<&Signal<T>> for Input<T, M> {
    fn from(value: &Signal<T>) -> Self {
        value.clone().into()
    }
}
impl<T, M> From<Computed<T, M>> for Input<T, M> {
    fn from(value: Computed<T, M>) -> Self {
        Self(motiongfx_authoring::Input::Computed(value))
    }
}
impl<T, M> From<&Computed<T, M>> for Input<T, M> {
    fn from(value: &Computed<T, M>) -> Self {
        value.clone().into()
    }
}
impl<
    M: Model2d,
    I: SubjectId,
    S: 'static,
    T: Clone + PartialEq + ThreadSafe,
> From<Property<I, S, T>> for Input<T, M>
where
    World<M>: SubjectSource<I, S>,
{
    fn from(value: Property<I, S, T>) -> Self {
        Self::mapped(value, std::convert::identity)
    }
}
impl<T: Clone + PartialEq + ThreadSafe, M: Model2d> Source<World<M>>
    for Input<T, M>
{
    type Value = T;
    type Owned = Self;
    type Prepared = motiongfx_authoring::PreparedInput<M, T>;
    fn owned(&self) -> Self {
        self.clone()
    }
    fn register(
        &self,
        world: &mut World<M>,
        plan: &mut BindingPlan<World<M>>,
    ) -> Self::Prepared {
        self.0.register(world, plan)
    }
}
impl<M: Model2d, I, S> From<crate::Position<I, S>> for Input<Point, M>
where
    I: SubjectId,
    S: 'static,
    World<M>: SubjectSource<I, S>,
{
    fn from(value: crate::Position<I, S>) -> Self {
        Self::mapped(value, std::convert::identity)
    }
}
impl<M: Model2d> From<(f64, f64)> for Input<Point, M> {
    fn from(value: (f64, f64)) -> Self {
        Self::fixed(value.into())
    }
}
impl<M: Model2d> From<Vec2> for Input<Point, M> {
    fn from(value: Vec2) -> Self {
        Self::fixed(Point::new(value.x, value.y))
    }
}
impl<M: Model2d> From<(f64, f64)> for Input<Size, M> {
    fn from(value: (f64, f64)) -> Self {
        Self::fixed(Size::new(value.0, value.1))
    }
}
impl<M: Model2d> From<&str> for Input<Arc<str>, M> {
    fn from(value: &str) -> Self {
        Self::fixed(value.into())
    }
}
impl<M: Model2d> From<String> for Input<Arc<str>, M> {
    fn from(value: String) -> Self {
        Self::fixed(value.into())
    }
}
impl<M: Model2d, const N: usize> From<[f64; N]>
    for Input<Arc<[f64]>, M>
{
    fn from(value: [f64; N]) -> Self {
        Self::fixed(Arc::from(value))
    }
}
impl<M: Model2d> From<Vec<f64>> for Input<Arc<[f64]>, M> {
    fn from(value: Vec<f64>) -> Self {
        Self::fixed(value.into())
    }
}
impl<M: Model2d> From<Computed<(f64, f64), M>> for Input<Point, M> {
    fn from(value: Computed<(f64, f64), M>) -> Self {
        Self::mapped(value, Point::from)
    }
}
impl<M: Model2d> From<Signal<(f64, f64)>> for Input<Point, M> {
    fn from(value: Signal<(f64, f64)>) -> Self {
        Self::mapped(value, Point::from)
    }
}
impl<M: Model2d> From<&Computed<(f64, f64), M>> for Input<Point, M> {
    fn from(value: &Computed<(f64, f64), M>) -> Self {
        Self::mapped(value, Point::from)
    }
}
impl<M: Model2d> From<&Signal<(f64, f64)>> for Input<Point, M> {
    fn from(value: &Signal<(f64, f64)>) -> Self {
        Self::mapped(value, Point::from)
    }
}
