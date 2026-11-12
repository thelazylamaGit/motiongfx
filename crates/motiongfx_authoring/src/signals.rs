//! Immutable signal descriptions and independent scene-local playback values.

use crate::source::SourceId;
use crate::{BindingPlan, Computed, Property, Source};
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
    pub fn to<M>(
        &self,
        value: impl Into<T>,
    ) -> SignalChange<T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Interpolation<M>,
    {
        let value = value.into();
        self.change(move |_| value.clone(), T::interp)
    }
    pub fn set(
        &self,
        value: impl Into<T>,
    ) -> SignalChange<T, impl Fn(&T) -> T + ThreadSafe> {
        let value = value.into();
        let mut change = self.change(
            move |_| value.clone(),
            |a, b, t| if t < 1.0 { a.clone() } else { b.clone() },
        );
        change.duration = Some(Duration::ZERO);
        change
    }
    pub fn offset<D, M>(
        &self,
        delta: D,
    ) -> SignalChange<T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Interpolation<M> + Add<D, Output = T>,
        D: Copy + ThreadSafe,
    {
        self.change(move |v| v.clone() + delta, T::interp)
    }
    pub fn multiply<D, M>(
        &self,
        factor: D,
    ) -> SignalChange<T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Interpolation<M> + Mul<D, Output = T>,
        D: Copy + ThreadSafe,
    {
        self.change(move |v| v.clone() * factor, T::interp)
    }
    fn change<F: Fn(&T) -> T + ThreadSafe>(
        &self,
        target: F,
        interp: InterpFn<T>,
    ) -> SignalChange<T, F> {
        SignalChange {
            signal: self.clone(),
            target,
            interp,
            duration: None,
            ease: None,
        }
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

#[must_use = "Schedule the signal change with sequence.play(...) or sequence.play_all(...)"]
pub struct SignalChange<T, F> {
    signal: Signal<T>,
    target: F,
    interp: InterpFn<T>,
    duration: Option<Duration>,
    ease: Option<EaseFn>,
}
impl<T, F> SignalChange<T, F> {
    pub fn over(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
    pub fn ease(mut self, ease: EaseFn) -> Self {
        self.ease = Some(ease);
        self
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
impl<W: 'static, C, T, F> crate::AnimationChange<W, C>
    for SignalChange<T, F>
where
    C: RegisterSignals,
    T: Clone + ThreadSafe,
    W: SubjectSource<SignalId, T>,
    F: Fn(&T) -> T + ThreadSafe,
{
    fn lower(
        self,
        sequence: &mut crate::Sequence<'_, W, C>,
    ) -> TrackFragment {
        let id = sequence.context.signals().register(&self.signal);
        let change = crate::Change {
            property: Property {
                id,
                path: path!(T),
                interp: self.interp,
            },
            target: self.target,
            duration: self.duration,
            ease: self.ease,
        };
        crate::AnimationChange::lower(change, sequence)
    }
}
