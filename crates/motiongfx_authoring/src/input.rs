//! [`Input`]: a fixed value or a source, accepted by object builder setters.

use crate::bindings::{BindingDependency, DerivedValues};
use crate::{
    BindingError, BindingPlan, BindingSource, Property, Source,
};
use crate::{Computed, Signal, World};
use motiongfx::prelude::{SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use std::sync::Arc;

/// A value for an object field: either fixed, or read from a signal,
/// computed value or another object's property each time the scene is
/// sampled. Builder setters take `impl Into<Input<..>>`, so callers pass
/// `10.0`, `&radius` or `radius.map(..)` directly.
pub enum Input<M, T> {
    /// A constant; it becomes the field's initial value.
    Fixed(T),
    /// Follows a signal.
    Signal(Signal<T>),
    /// Follows a computed value.
    Computed(Computed<World<M>, T>),
    #[doc(hidden)]
    Reader(Reader<M, T>),
}
impl<M, T: Clone> Clone for Input<M, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Fixed(v) => Self::Fixed(v.clone()),
            Self::Signal(v) => Self::Signal(v.clone()),
            Self::Computed(v) => Self::Computed(v.clone()),
            Self::Reader(v) => Self::Reader(v.clone()),
        }
    }
}
impl<M, T> Input<M, T> {
    /// The value, if this input is fixed.
    pub fn as_fixed(&self) -> Option<&T> {
        if let Self::Fixed(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
// A registered source plus its conversion: reads the converted value and
// checks the source exists. One object for both, so a prepared reader holds
// a single shared pointer.
trait MappedRead<M, T>: Send + Sync {
    fn read(
        &self,
        world: &World<M>,
        values: &DerivedValues,
    ) -> Option<T>;
    fn validate(&self, world: &World<M>) -> Result<(), BindingError>;
}
struct Mapped<P, F> {
    source: P,
    convert: Arc<F>,
}
impl<M, T, P, F> MappedRead<M, T> for Mapped<P, F>
where
    P: BindingSource<World<M>>,
    F: Fn(P::Value) -> T + Send + Sync,
{
    fn read(
        &self,
        world: &World<M>,
        values: &DerivedValues,
    ) -> Option<T> {
        self.source.read(world, values).map(&*self.convert)
    }
    fn validate(&self, world: &World<M>) -> Result<(), BindingError> {
        self.source.validate(world)
    }
}
#[doc(hidden)]
pub struct Reader<M, T> {
    register: Arc<Register<M, T>>,
}
impl<M, T> Clone for Reader<M, T> {
    fn clone(&self) -> Self {
        Self {
            register: self.register.clone(),
        }
    }
}
type Register<M, T> = dyn Fn(
        &mut World<M>,
        &mut BindingPlan<World<M>>,
    ) -> PreparedReader<M, T>
    + Send
    + Sync;
#[doc(hidden)]
pub struct PreparedReader<M, T> {
    reader: Arc<dyn MappedRead<M, T>>,
    dependencies: Vec<BindingDependency>,
}
impl<M, T> Clone for PreparedReader<M, T> {
    fn clone(&self) -> Self {
        Self {
            reader: self.reader.clone(),
            dependencies: self.dependencies.clone(),
        }
    }
}
impl<M: 'static, T: Clone + PartialEq + ThreadSafe> Input<M, T> {
    /// Follow `source`, converting each value with `convert`; for example a
    /// computed `(f64, f64)` tuple used as a `Point`.
    pub fn mapped<S, F>(source: S, convert: F) -> Self
    where
        S: Source<World<M>>,
        F: Fn(S::Value) -> T + ThreadSafe,
    {
        let source = source.owned();
        let convert = Arc::new(convert);
        Self::Reader(Reader {
            register: Arc::new(move |world, plan| {
                let source = source.register(world, plan);
                PreparedReader {
                    dependencies: source.dependencies(),
                    reader: Arc::new(Mapped {
                        source,
                        convert: convert.clone(),
                    }),
                }
            }),
        })
    }
}
impl<M: 'static, T> From<T> for Input<M, T> {
    fn from(value: T) -> Self {
        Self::Fixed(value)
    }
}
impl<M: 'static, T> From<Signal<T>> for Input<M, T> {
    fn from(value: Signal<T>) -> Self {
        Self::Signal(value)
    }
}
impl<M: 'static, T> From<Computed<World<M>, T>> for Input<M, T> {
    fn from(value: Computed<World<M>, T>) -> Self {
        Self::Computed(value)
    }
}
impl<M: 'static, I, S, T> From<Property<I, S, T>> for Input<M, T>
where
    I: SubjectId,
    S: 'static,
    T: Clone + PartialEq + ThreadSafe,
    World<M>: SubjectSource<I, S>,
{
    fn from(value: Property<I, S, T>) -> Self {
        Self::mapped(value, std::convert::identity)
    }
}
impl<M: 'static, T: Clone + PartialEq + ThreadSafe>
    BindingSource<World<M>> for PreparedInput<M, T>
{
    type Value = T;
    fn read(
        &self,
        world: &World<M>,
        values: &DerivedValues,
    ) -> Option<T> {
        match self {
            Self::Fixed(value) => Some(value.clone()),
            Self::Signal(value) => value.read(world, values),
            Self::Computed(value) => value.read(world, values),
            Self::Reader(value) => value.reader.read(world, values),
        }
    }
    fn dependencies(&self) -> Vec<BindingDependency> {
        match self {
            Self::Fixed(_) => Vec::new(),
            Self::Signal(value) => {
                <Property<crate::SignalId, T, T> as BindingSource<
                    World<M>,
                >>::dependencies(value)
            }
            Self::Computed(value) => {
                <crate::bindings::ComputedSlot<T> as BindingSource<
                    World<M>,
                >>::dependencies(value)
            }
            Self::Reader(value) => value.dependencies.clone(),
        }
    }
    fn validate(&self, world: &World<M>) -> Result<(), BindingError> {
        match self {
            Self::Fixed(_) | Self::Computed(_) => Ok(()),
            Self::Signal(value) => value.validate(world),
            Self::Reader(value) => value.reader.validate(world),
        }
    }
}

impl<M: 'static, T: Clone> From<&Signal<T>> for Input<M, T> {
    fn from(value: &Signal<T>) -> Self {
        Self::Signal(value.clone())
    }
}
impl<M: 'static, T> From<&Computed<World<M>, T>> for Input<M, T> {
    fn from(value: &Computed<World<M>, T>) -> Self {
        Self::Computed(value.clone())
    }
}
/// An [`Input`] registered in a scene, read while sampling.
#[doc(hidden)]
pub enum PreparedInput<M, T> {
    Fixed(T),
    Signal(Property<crate::SignalId, T, T>),
    Computed(crate::bindings::ComputedSlot<T>),
    Reader(PreparedReader<M, T>),
}
impl<M, T: Clone> Clone for PreparedInput<M, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Fixed(v) => Self::Fixed(v.clone()),
            Self::Signal(v) => Self::Signal(*v),
            Self::Computed(v) => Self::Computed(*v),
            Self::Reader(v) => Self::Reader(v.clone()),
        }
    }
}
impl<M: 'static, T: Clone + PartialEq + ThreadSafe> Source<World<M>>
    for Input<M, T>
{
    type Value = T;
    type Owned = Self;
    type Prepared = PreparedInput<M, T>;
    fn owned(&self) -> Self {
        self.clone()
    }
    fn register(
        &self,
        world: &mut World<M>,
        plan: &mut BindingPlan<World<M>>,
    ) -> Self::Prepared {
        match self {
            Self::Fixed(value) => PreparedInput::Fixed(value.clone()),
            Self::Signal(value) => {
                PreparedInput::Signal(value.register(world, plan))
            }
            Self::Computed(value) => {
                PreparedInput::Computed(value.register(world, plan))
            }
            Self::Reader(value) => {
                PreparedInput::Reader((value.register)(world, plan))
            }
        }
    }
}

impl<
    M: 'static,
    I: SubjectId,
    S: 'static,
    T: Clone + PartialEq + ThreadSafe,
    F: crate::Field<S, T>,
> From<crate::Target<I, S, T, F>> for Input<M, T>
where
    World<M>: SubjectSource<I, S>,
{
    fn from(value: crate::Target<I, S, T, F>) -> Self {
        Self::mapped(value, std::convert::identity)
    }
}
