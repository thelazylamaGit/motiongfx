//! Sources: values that bindings and object inputs can read, such as
//! properties, signals and computed values.

use crate::{BindingPlan, BindingSource, Property};
use motiongfx::prelude::{SubjectSource, ThreadSafe};
use motiongfx::subject::SubjectId;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Identifies one signal or computed description, so each scene registers
/// it once however many times it is used.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceId(u64);
impl SourceId {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Something a binding or object input can read: a property, a signal, a
/// computed value, the sample time, or a tuple of up to five of these.
///
/// A source is a description. Each scene registers it once, getting a
/// [`Prepared`](Self::Prepared) reader that sampling uses.
pub trait Source<W> {
    /// The value read when sampling.
    type Value: Clone + PartialEq + ThreadSafe;
    /// The reader registered in a scene.
    type Prepared: BindingSource<W, Value = Self::Value>;
    /// An owned copy, for computed values that keep their inputs.
    type Owned: Source<W, Value = Self::Value, Prepared = Self::Prepared>
        + Clone
        + ThreadSafe;
    /// Copy this description into an owned one.
    fn owned(&self) -> Self::Owned;
    /// Register this source with `world` and `plan`, once per scene.
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

/// A value calculated from other sources, such as a point on a curve
/// that follows a signal. Build one with [`computed`], [`computed!`] or
/// `.map(..)`.
///
/// A computed value is a description: each scene that uses it caches its
/// result and recalculates only when its inputs change. Clones share the
/// same description, so the calculation runs once per scene however many
/// objects read it.
///
/// [`computed!`]: crate::computed!
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
    /// Calculate a value from `inputs` (one source or a tuple of them).
    /// `calculate` receives the inputs' values, as a tuple for several.
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
    /// A value calculated from this one.
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

/// A value calculated from `inputs`; shorthand for [`Computed::new`].
///
/// ```
/// use motiongfx_authoring::{computed, signal};
/// let width = signal(4.0);
/// let height = signal(3.0);
/// let area = computed((&width, &height), |(w, h)| w * h);
/// # let _: motiongfx_authoring::Computed<motiongfx_authoring::World<()>, f64> = area;
/// ```
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
