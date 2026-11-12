//! Fixed values and typed sources accepted by authoring descriptors.

use crate::{Computed, Graphics, Point, Signal, Size, Vec2};
use motiongfx::prelude::{SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use motiongfx_authoring::bindings::{
    BindingDependency, DerivedValues,
};
use motiongfx_authoring::{BindingError, BindingSource, Property};
use std::sync::Arc;

/// Fixed inputs create no binding. Sources use the existing dependency plan;
/// they are not reactive cells embedded in the stored object's state.
#[derive(Clone)]
pub enum Input<T> {
    Fixed(T),
    Signal(Signal<T>),
    Computed(Computed<T>),
    #[doc(hidden)]
    Reader(Reader<T>),
}
type Read<T> =
    dyn Fn(&Graphics, &DerivedValues) -> Option<T> + Send + Sync;
type Validate =
    dyn Fn(&Graphics) -> Result<(), BindingError> + Send + Sync;
#[doc(hidden)]
#[derive(Clone)]
pub struct Reader<T> {
    read: Arc<Read<T>>,
    validate: Arc<Validate>,
    dependencies: Vec<BindingDependency>,
}
impl<T: Clone + PartialEq + ThreadSafe> Input<T> {
    fn mapped<S, F>(source: S, convert: F) -> Self
    where
        S: BindingSource<Graphics>,
        F: Fn(S::Value) -> T + ThreadSafe,
    {
        let validation_source = source.clone();
        Self::Reader(Reader {
            dependencies: source.dependencies(),
            read: Arc::new(move |world, values| {
                source.read(world, values).map(&convert)
            }),
            validate: Arc::new(move |world| {
                validation_source.validate(world)
            }),
        })
    }
}
impl<T> From<T> for Input<T> {
    fn from(value: T) -> Self {
        Self::Fixed(value)
    }
}
impl<T> From<Signal<T>> for Input<T> {
    fn from(value: Signal<T>) -> Self {
        Self::Signal(value)
    }
}
impl<T> From<Computed<T>> for Input<T> {
    fn from(value: Computed<T>) -> Self {
        Self::Computed(value)
    }
}
impl<I, S, T> From<Property<I, S, T>> for Input<T>
where
    I: SubjectId,
    S: 'static,
    T: Clone + PartialEq + ThreadSafe,
    Graphics: SubjectSource<I, S>,
{
    fn from(value: Property<I, S, T>) -> Self {
        Self::mapped(value, std::convert::identity)
    }
}
impl<I, S> From<crate::Position<I, S>> for Input<Point>
where
    I: SubjectId,
    S: 'static,
    Graphics: SubjectSource<I, S>,
{
    fn from(value: crate::Position<I, S>) -> Self {
        Self::mapped(value, std::convert::identity)
    }
}
impl From<(f64, f64)> for Input<Point> {
    fn from(value: (f64, f64)) -> Self {
        Self::Fixed(value.into())
    }
}
impl From<Vec2> for Input<Point> {
    fn from(value: Vec2) -> Self {
        Self::Fixed(Point::new(value.x, value.y))
    }
}
impl From<(f64, f64)> for Input<Size> {
    fn from(value: (f64, f64)) -> Self {
        Self::Fixed(Size::new(value.0, value.1))
    }
}
impl From<&str> for Input<Arc<str>> {
    fn from(value: &str) -> Self {
        Self::Fixed(value.into())
    }
}
impl From<String> for Input<Arc<str>> {
    fn from(value: String) -> Self {
        Self::Fixed(value.into())
    }
}
impl<const N: usize> From<[f64; N]> for Input<Arc<[f64]>> {
    fn from(value: [f64; N]) -> Self {
        Self::Fixed(Arc::from(value))
    }
}
impl From<Vec<f64>> for Input<Arc<[f64]>> {
    fn from(value: Vec<f64>) -> Self {
        Self::Fixed(value.into())
    }
}
impl From<Computed<(f64, f64)>> for Input<Point> {
    fn from(value: Computed<(f64, f64)>) -> Self {
        Self::mapped(value, Point::from)
    }
}
impl From<Signal<(f64, f64)>> for Input<Point> {
    fn from(value: Signal<(f64, f64)>) -> Self {
        Self::mapped(value, Point::from)
    }
}
impl<T: Clone + PartialEq + ThreadSafe> BindingSource<Graphics>
    for Input<T>
{
    type Value = T;
    fn read(
        &self,
        world: &Graphics,
        values: &DerivedValues,
    ) -> Option<T> {
        match self {
            Self::Fixed(value) => Some(value.clone()),
            Self::Signal(value) => value.read(world, values),
            Self::Computed(value) => value.read(world, values),
            Self::Reader(value) => (value.read)(world, values),
        }
    }
    fn dependencies(&self) -> Vec<BindingDependency> {
        match self {
            Self::Fixed(_) => Vec::new(),
            Self::Signal(value) => value.dependencies(),
            Self::Computed(value) => {
                <Computed<T> as BindingSource<Graphics>>::dependencies(
                    value,
                )
            }
            Self::Reader(value) => value.dependencies.clone(),
        }
    }
    fn validate(&self, world: &Graphics) -> Result<(), BindingError> {
        match self {
            Self::Fixed(_) | Self::Computed(_) => Ok(()),
            Self::Signal(value) => value.validate(world),
            Self::Reader(value) => (value.validate)(world),
        }
    }
}
