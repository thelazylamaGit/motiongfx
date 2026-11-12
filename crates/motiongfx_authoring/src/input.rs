//! Fixed values and typed sources accepted by authoring descriptors.

use crate::bindings::{BindingDependency, DerivedValues};
use crate::{
    BindingError, BindingPlan, BindingSource, Property, Source,
};
use crate::{Computed, Signal, World};
use motiongfx::prelude::{SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use std::sync::Arc;

/// Fixed inputs create no binding. Sources use the existing dependency plan;
/// they are not reactive cells embedded in the stored object's state.
pub enum Input<M, T> {
    Fixed(T),
    Signal(Signal<T>),
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
    pub fn as_fixed(&self) -> Option<&T> {
        if let Self::Fixed(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
type Read<M, T> =
    dyn Fn(&World<M>, &DerivedValues) -> Option<T> + Send + Sync;
type Validate<M> =
    dyn Fn(&World<M>) -> Result<(), BindingError> + Send + Sync;
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
    read: Arc<Read<M, T>>,
    validate: Arc<Validate<M>>,
    dependencies: Vec<BindingDependency>,
}
impl<M, T> Clone for PreparedReader<M, T> {
    fn clone(&self) -> Self {
        Self {
            read: self.read.clone(),
            validate: self.validate.clone(),
            dependencies: self.dependencies.clone(),
        }
    }
}
impl<M: 'static, T: Clone + PartialEq + ThreadSafe> Input<M, T> {
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
                let validation_source = source.clone();
                let convert = convert.clone();
                PreparedReader {
                    dependencies: source.dependencies(),
                    read: Arc::new(move |world, values| {
                        source.read(world, values).map(&*convert)
                    }),
                    validate: Arc::new(move |world| {
                        validation_source.validate(world)
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
            Self::Reader(value) => (value.read)(world, values),
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
            Self::Reader(value) => (value.validate)(world),
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
/// Prepared once during scene construction; playback uses indexed sources.
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
