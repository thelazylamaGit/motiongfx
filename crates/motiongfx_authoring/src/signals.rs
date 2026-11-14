//! Immutable signal descriptions and independent scene-local playback values.

use crate::source::SourceId;
use crate::{Animation, BindingPlan, Computed, Property, Source};
use motiongfx::prelude::*;
use std::time::Duration;
use std::{
    any::Any,
    collections::HashMap,
    ops::{Add, Mul},
    sync::Arc,
};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct SignalId(usize);

struct Initial<T> {
    id: SourceId,
    value: T,
}
/// A standalone writable animation input. Clones share the description,
/// not mutable playback state. Methods borrow the handle rather than consume it.
pub struct Signal<T>(Arc<Initial<T>>);
impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Clone + ThreadSafe> Signal<T> {
    pub fn new(initial: T) -> Self {
        Self(Arc::new(Initial {
            id: SourceId::new(),
            value: initial,
        }))
    }
    /// Animate to `value`. (`Marker` selects the interpolation impl and
    /// is inferred.)
    #[track_caller]
    pub fn to<M: 'static, Marker>(
        &self,
        value: impl Into<T>,
    ) -> Animation<M>
    where
        T: Interpolation<Marker>,
    {
        let value = value.into();
        self.change(move |_| value.clone(), T::interp, false)
    }
    /// Replace the value instantly at the point where this is scheduled.
    #[track_caller]
    pub fn set<M: 'static>(
        &self,
        value: impl Into<T>,
    ) -> Animation<M> {
        let value = value.into();
        self.change(
            move |_| value.clone(),
            |a, b, t| if t < 1.0 { a.clone() } else { b.clone() },
            true,
        )
        .over(Duration::ZERO)
    }
    /// Animate by adding `delta` to the value at the change's start.
    #[track_caller]
    pub fn offset<M: 'static, D, Marker>(
        &self,
        delta: D,
    ) -> Animation<M>
    where
        T: Interpolation<Marker> + Add<D, Output = T>,
        D: Copy + ThreadSafe,
    {
        self.change(move |v: &T| v.clone() + delta, T::interp, false)
    }
    /// Animate by multiplying the value at the change's start by `factor`.
    #[track_caller]
    pub fn multiply<M: 'static, D, Marker>(
        &self,
        factor: D,
    ) -> Animation<M>
    where
        T: Interpolation<Marker> + Mul<D, Output = T>,
        D: Copy + ThreadSafe,
    {
        self.change(move |v: &T| v.clone() * factor, T::interp, false)
    }
    /// Shorthand for `.to(value).spring(spring)`.
    #[track_caller]
    pub fn spring_to<M: 'static, Marker>(
        &self,
        value: impl Into<T>,
        spring: impl Into<crate::Spring>,
    ) -> Animation<M>
    where
        T: Interpolation<Marker>,
    {
        self.to(value).spring(spring)
    }
    #[track_caller]
    fn change<M: 'static>(
        &self,
        target: impl Fn(&T) -> T + Clone + ThreadSafe,
        interp: InterpFn<T>,
        discrete: bool,
    ) -> Animation<M> {
        Animation::from_step(SignalStep {
            signal: self.clone(),
            target,
            interp,
            discrete,
        })
    }
    pub fn map<W, U, F>(&self, calculate: F) -> Computed<W, U>
    where
        W: 'static + SignalWorld + SubjectSource<SignalId, T>,
        T: PartialEq,
        U: Clone + PartialEq + ThreadSafe,
        F: Fn(T) -> U + ThreadSafe,
    {
        Computed::new(self, calculate)
    }
}
pub fn signal<T: Clone + ThreadSafe>(initial: T) -> Signal<T> {
    Signal::new(initial)
}

/// Construction-only lookup maps stable description identities to dense slots.
/// Sampling uses the slots directly and never traverses the descriptions.
#[derive(Default)]
pub struct SignalValues {
    ids: HashMap<SourceId, SignalId>,
    values: Vec<Box<dyn Any + Send + Sync>>,
}
impl SignalValues {
    pub fn registered_value<T: ThreadSafe>(
        &self,
        signal: &Signal<T>,
    ) -> Option<&T> {
        self.get(*self.ids.get(&signal.0.id)?)
    }
    pub fn register<T: Clone + ThreadSafe>(
        &mut self,
        signal: &Signal<T>,
    ) -> SignalId {
        if let Some(id) = self.ids.get(&signal.0.id) {
            return *id;
        }
        let id = SignalId(self.values.len());
        self.values.push(Box::new(signal.0.value.clone()));
        self.ids.insert(signal.0.id, id);
        id
    }
    pub fn get<T: ThreadSafe>(&self, id: SignalId) -> Option<&T> {
        self.values.get(id.0)?.downcast_ref()
    }
    pub fn apply<T: ThreadSafe, R>(
        &mut self,
        id: SignalId,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        Some(f(self.values.get_mut(id.0)?.downcast_mut()?))
    }
}
pub trait SignalWorld {
    fn signals_mut(&mut self) -> &mut SignalValues;
}
impl<W, T> Source<W> for Signal<T>
where
    W: SignalWorld + SubjectSource<SignalId, T>,
    T: Clone + PartialEq + ThreadSafe,
{
    type Value = T;
    type Prepared = Property<SignalId, T, T>;
    type Owned = Self;
    fn owned(&self) -> Self {
        self.clone()
    }
    fn register(
        &self,
        world: &mut W,
        _: &mut BindingPlan<W>,
    ) -> Self::Prepared {
        Property::discrete(
            world.signals_mut().register(self),
            path!(T),
        )
    }
}

/// A sequence capability: roots used only in timeline actions still register.
pub trait RegisterSignals {
    fn signals(&mut self) -> &mut SignalValues;
}
impl RegisterSignals for &mut SignalValues {
    fn signals(&mut self) -> &mut SignalValues {
        self
    }
}

/// A signal change. The signal gets its scene-local slot when lowered, so
/// signals can be described before or without a scene.
struct SignalStep<T, F> {
    signal: Signal<T>,
    target: F,
    interp: InterpFn<T>,
    discrete: bool,
}
impl<M, T, F> crate::Step<M> for SignalStep<T, F>
where
    M: 'static,
    T: Clone + ThreadSafe,
    F: Fn(&T) -> T + Clone + ThreadSafe,
{
    fn lower(
        &self,
        sequence: &mut crate::Sequence<
            '_,
            crate::World<M>,
            crate::SceneContext<'_, M>,
        >,
        timing: &crate::Timing,
    ) -> TrackFragment {
        let id =
            sequence.context_mut().signals().register(&self.signal);
        let property = Property {
            id,
            path: path!(T),
            interp: self.interp,
            discrete: self.discrete,
        };
        crate::lower_change(
            sequence,
            property,
            self.target.clone(),
            timing,
        )
    }
}
