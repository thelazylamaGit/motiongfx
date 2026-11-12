//! Scene-independent authoring sources lower to indexed playback sources.

use crate::{BindingPlan, BindingSource, Property};
use motiongfx::prelude::{SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Identity of an immutable authoring description, never sampled directly.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceId(u64);
impl SourceId {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// A description that can be registered independently in each playback world.
/// Borrowed sources are converted to owned descriptions when retained by a
/// computed. Registration is construction work; Prepared is used in playback.
pub trait Source<W> {
    type Value: Clone + PartialEq + ThreadSafe;
    type Prepared: BindingSource<W, Value = Self::Value>;
    type Owned: Source<W, Value = Self::Value, Prepared = Self::Prepared>
        + Clone
        + ThreadSafe;
    fn owned(&self) -> Self::Owned;
    fn register(
        &self,
        world: &mut W,
        plan: &mut BindingPlan<W>,
    ) -> Self::Prepared;
}
impl<W, S: Source<W>> Source<W> for &S {
    type Value = S::Value;
    type Prepared = S::Prepared;
    type Owned = S::Owned;
    fn owned(&self) -> Self::Owned {
        S::owned(self)
    }
    fn register(
        &self,
        world: &mut W,
        plan: &mut BindingPlan<W>,
    ) -> Self::Prepared {
        S::register(self, world, plan)
    }
}
impl<W, I, S, T> Source<W> for Property<I, S, T>
where
    I: SubjectId,
    S: 'static,
    T: Clone + PartialEq + ThreadSafe,
    W: SubjectSource<I, S>,
{
    type Value = T;
    type Prepared = Self;
    type Owned = Self;
    fn owned(&self) -> Self {
        *self
    }
    fn register(&self, _: &mut W, _: &mut BindingPlan<W>) -> Self {
        *self
    }
}

/// Standalone read-only description of one shared native calculation.
/// Each scene registers and caches it independently. No live value or scene
/// reference is stored here, and no evaluation takes place in this constructor.
pub struct Computed<W, T> {
    id: SourceId,
    register: Arc<Register<W, T>>,
}
type Register<W, T> = dyn Fn(
        &mut W,
        &mut BindingPlan<W>,
    ) -> crate::bindings::ComputedSlot<T>
    + Send
    + Sync;
impl<W, T> Clone for Computed<W, T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            register: self.register.clone(),
        }
    }
}
impl<W: 'static, T: Clone + PartialEq + ThreadSafe> Computed<W, T> {
    pub fn new<S, F>(inputs: S, calculate: F) -> Self
    where
        S: Source<W>,
        F: Fn(S::Value) -> T + ThreadSafe,
    {
        let inputs = inputs.owned();
        let calculate = Arc::new(calculate);
        Self {
            id: SourceId::new(),
            register: Arc::new(move |world, plan| {
                let prepared = inputs.register(world, plan);
                let calculate = calculate.clone();
                plan.computed(prepared, move |input| calculate(input))
            }),
        }
    }
    pub fn map<U, F>(&self, calculate: F) -> Computed<W, U>
    where
        U: Clone + PartialEq + ThreadSafe,
        F: Fn(T) -> U + ThreadSafe,
    {
        Computed::new(self, calculate)
    }
}
impl<W: 'static, T: Clone + PartialEq + ThreadSafe> Source<W>
    for Computed<W, T>
{
    type Value = T;
    type Prepared = crate::bindings::ComputedSlot<T>;
    type Owned = Self;
    fn owned(&self) -> Self {
        self.clone()
    }
    fn register(
        &self,
        world: &mut W,
        plan: &mut BindingPlan<W>,
    ) -> Self::Prepared {
        if let Some(slot) = plan.registered_computed(self.id) {
            return slot;
        }
        let slot = (self.register)(world, plan);
        plan.remember_computed(self.id, slot);
        slot
    }
}

/// Convenience constructor for Computed::new, with identical execution.
pub fn computed<W: 'static, S, T, F>(
    inputs: S,
    calculate: F,
) -> Computed<W, T>
where
    S: Source<W>,
    T: Clone + PartialEq + ThreadSafe,
    F: Fn(S::Value) -> T + ThreadSafe,
{
    Computed::new(inputs, calculate)
}

macro_rules! tuple_source {
    ($($name:ident:$index:tt),+) => {
        impl<W, $($name: Source<W>),+> Source<W> for ($($name,)+) {
            type Value = ($($name::Value,)+);
            type Prepared = ($($name::Prepared,)+);
            type Owned = ($($name::Owned,)+);
            fn owned(&self) -> Self::Owned { ($(self.$index.owned(),)+) }
            fn register(&self, world: &mut W, plan: &mut BindingPlan<W>) -> Self::Prepared {
                ($(self.$index.register(world, plan),)+)
            }
        }
    }
}
tuple_source!(A:0);
tuple_source!(A:0, B:1);
tuple_source!(A:0, B:1, C:2);
tuple_source!(A:0, B:1, C:2, D:3);
tuple_source!(A:0, B:1, C:2, D:3, E:4);
