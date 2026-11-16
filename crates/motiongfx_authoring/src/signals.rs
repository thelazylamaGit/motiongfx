//! Signals: animatable values that aren't part of any object, such as a
//! progress value several objects follow.

use crate::source::SourceId;
use crate::{
    Animation, BindingPlan, Computed, Interp, Property, Source,
};
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
/// A signal's slot in one scene's [`SignalValues`].
pub struct SignalId(usize);

struct Initial<T> {
    id: SourceId,
    value: T,
}
/// An animatable value that isn't part of any object, such as a progress
/// value that several objects follow. Create one with [`signal`].
///
/// A signal is a description: each scene that uses it keeps its own current
/// value, starting from the initial one. Clones refer to the same signal.
///
/// ```
/// use motiongfx::prelude::ms;
/// use motiongfx_authoring::{Animation, signal};
/// let progress = signal(0.0);
/// // Changes to signals are animations like any other.
/// let fill: Animation<()> = progress.to(1.0).over(ms(500));
/// ```
pub struct Signal<T>(Arc<Initial<T>>);
/// Shows the signal's initial value.
impl<T: std::fmt::Debug> std::fmt::Debug for Signal<T> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_tuple("Signal").field(&self.0.value).finish()
    }
}
impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Clone + ThreadSafe> Signal<T> {
    /// A signal starting at `initial`.
    pub fn new(initial: T) -> Self {
        Self(Arc::new(Initial {
            id: SourceId::new(),
            value: initial,
        }))
    }
    /// Animate to `value`. (`Marker` picks how `T` interpolates and is
    /// inferred.)
    #[track_caller]
    pub fn to<M: 'static, Marker>(
        &self,
        value: impl Into<T>,
    ) -> Animation<M>
    where
        T: Interpolation<Marker>,
    {
        let value = value.into();
        self.change(move |_| value.clone(), Interp::Smooth(T::interp))
    }
    /// Replace the value instantly at the point where this is scheduled.
    #[track_caller]
    pub fn set<M: 'static>(
        &self,
        value: impl Into<T>,
    ) -> Animation<M> {
        let value = value.into();
        self.change(move |_| value.clone(), Interp::Discrete)
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
        self.change(
            move |v: &T| v.clone() + delta,
            Interp::Smooth(T::interp),
        )
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
        self.change(
            move |v: &T| v.clone() * factor,
            Interp::Smooth(T::interp),
        )
    }
    #[track_caller]
    fn change<M: 'static>(
        &self,
        target: impl Fn(&T) -> T + Clone + ThreadSafe,
        interp: Interp<T>,
    ) -> Animation<M> {
        Animation::from_step(SignalStep {
            signal: self.clone(),
            target,
            interp,
        })
    }
    /// A value calculated from this signal, for objects to follow.
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
/// A signal starting at `initial`; shorthand for [`Signal::new`].
pub fn signal<T: Clone + ThreadSafe>(initial: T) -> Signal<T> {
    Signal::new(initial)
}

/// The current value of every signal one scene uses, each in its own slot.
#[derive(Default)]
pub struct SignalValues {
    ids: HashMap<SourceId, SignalId>,
    values: Vec<Box<dyn Any + Send + Sync>>,
}
impl SignalValues {
    /// The current value of `signal`, or `None` if it was never registered.
    pub fn registered_value<T: ThreadSafe>(
        &self,
        signal: &Signal<T>,
    ) -> Option<&T> {
        self.get(*self.ids.get(&signal.0.id)?)
    }
    /// Give `signal` a slot holding its initial value, or return the slot
    /// it already has.
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
    /// The value in slot `id`, if it exists and holds a `T`.
    pub fn get<T: ThreadSafe>(&self, id: SignalId) -> Option<&T> {
        self.values.get(id.0)?.downcast_ref()
    }
    /// The value in slot `id`, mutably, if it exists and holds a `T`.
    pub fn get_mut<T: ThreadSafe>(
        &mut self,
        id: SignalId,
    ) -> Option<&mut T> {
        self.values.get_mut(id.0)?.downcast_mut()
    }
    /// Run `f` on the value in slot `id`, if it exists and holds a `T`.
    pub fn apply<T: ThreadSafe, R>(
        &mut self,
        id: SignalId,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        Some(f(self.values.get_mut(id.0)?.downcast_mut()?))
    }
}
/// A world that stores signal values, so signals can be read in it.
pub trait SignalWorld {
    /// The signal values.
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

/// A change to a signal. The signal gets its slot in the scene when the
/// scene compiles, so signals can be created without a scene.
struct SignalStep<T, F> {
    signal: Signal<T>,
    target: F,
    interp: Interp<T>,
}
impl<M, T, F> crate::Step<M> for SignalStep<T, F>
where
    M: 'static,
    T: Clone + ThreadSafe,
    F: Fn(&T) -> T + Clone + ThreadSafe,
{
    fn lower(
        &self,
        context: &mut crate::CompileContext<'_, M>,
        timing: &crate::Timing,
    ) -> TrackFragment {
        let id = context.signals().register(&self.signal);
        let property = Property {
            id,
            path: path!(T),
            interp: self.interp,
        };
        crate::lower_change(
            context,
            property,
            self.target.clone(),
            timing,
        )
    }
}
