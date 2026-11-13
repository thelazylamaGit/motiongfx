//! Thin spring descriptions lowering to the core spring action primitive.
use crate::{
    AnimationChange, Property, RegisterSignals, Sequence, Signal,
    SignalId,
};
use core::marker::PhantomData;
use motiongfx::prelude::*;
pub use motiongfx::spring::{
    Spring, SpringCurve, SpringError, SpringSample, SpringValue,
};
use motiongfx::subject::SubjectId;

/// A target calculation with spring timing. No tween duration/easing methods.
#[must_use = "schedule the spring change with sequence.play"]
pub struct SpringTargetChange<I, S, T, F, M> {
    change: crate::Change<I, S, T, F>,
    spring: Spring,
    marker: PhantomData<fn() -> M>,
}
impl<I, S, T, F> crate::Change<I, S, T, F> {
    /// Replace default tween timing with a spring. Explicit over/ease conflict.
    /// ```compile_fail
    /// use motiongfx_authoring::{Property, Spring};
    /// let p = Property::<u32,f64,f64>::new(0,motiongfx::path!(f64));
    /// p.to(1.0).spring(Spring::physical()).over(std::time::Duration::from_secs(1));
    /// ```
    pub fn spring<M>(
        self,
        spring: impl Into<Spring>,
    ) -> SpringTargetChange<I, S, T, F, M>
    where
        T: SpringValue<M>,
    {
        SpringTargetChange {
            change: self,
            spring: spring.into(),
            marker: PhantomData,
        }
    }
}
impl<W: 'static, C, I, S, T, F, M> AnimationChange<W, C>
    for SpringTargetChange<I, S, T, F, M>
where
    I: SubjectId,
    S: Clone + ThreadSafe,
    T: SpringValue<M> + Clone + ThreadSafe,
    F: Fn(&T) -> T + ThreadSafe,
    W: SubjectSource<I, S>,
{
    fn lower(
        self,
        sequence: &mut Sequence<'_, W, C>,
    ) -> TrackFragment {
        if self.change.duration.is_some()
            || self.change.ease.is_some()
        {
            sequence.preparation_error.get_or_insert(
                crate::PreparationError::ConflictingSpringTiming,
            );
            return TrackFragment::silent(core::time::Duration::ZERO);
        }
        let result = sequence
            .builder
            .spring::<I, S, T, M>(
                self.change.property.id,
                self.change.property.path,
                self.change.target,
                self.spring,
            )
            .try_play();
        match result {
            Ok(fragment) => fragment,
            Err(error) => {
                sequence
                    .preparation_error
                    .get_or_insert(error.into());
                TrackFragment::silent(core::time::Duration::ZERO)
            }
        }
    }
}

#[must_use = "schedule the spring with sequence.play(...)"]
pub struct SpringChange<I, S, T, M> {
    property: Property<I, S, T>,
    value: T,
    spring: Spring,
    marker: PhantomData<fn() -> M>,
}
impl<I: Copy, S, T> Property<I, S, T> {
    /// A fixed-parameter spring; deliberately has no `.over`/`.ease` setters.
    /// Continuous values opt in through SpringValue; discrete values cannot spring.
    /// ```compile_fail
    /// use motiongfx_authoring::{Property,Spring};
    /// let p = Property::<u32,f64,f64>::new(0,motiongfx::path!(f64));
    /// p.spring_to(1.0,Spring::physical()).over(std::time::Duration::from_secs(1));
    /// ```
    pub fn spring_to<M>(
        self,
        value: impl Into<T>,
        spring: impl Into<Spring>,
    ) -> SpringChange<I, S, T, M>
    where
        T: SpringValue<M> + Clone + ThreadSafe,
    {
        SpringChange {
            property: self,
            value: value.into(),
            spring: spring.into(),
            marker: PhantomData,
        }
    }
}
impl<W: 'static, C, I, S, T, M> AnimationChange<W, C>
    for SpringChange<I, S, T, M>
where
    I: SubjectId,
    S: Clone + ThreadSafe,
    T: SpringValue<M> + Clone + ThreadSafe,
    W: SubjectSource<I, S>,
{
    fn lower(
        self,
        sequence: &mut Sequence<'_, W, C>,
    ) -> TrackFragment {
        self.property
            .to(self.value)
            .spring::<M>(self.spring)
            .lower(sequence)
    }
}
#[must_use = "schedule the signal spring with sequence.play(...)"]
pub struct SignalSpringChange<T, M> {
    signal: Signal<T>,
    value: T,
    spring: Spring,
    marker: PhantomData<fn() -> M>,
}
impl<T: Clone + ThreadSafe> Signal<T> {
    pub fn spring_to<M>(
        &self,
        value: impl Into<T>,
        spring: impl Into<Spring>,
    ) -> SignalSpringChange<T, M>
    where
        T: SpringValue<M>,
    {
        SignalSpringChange {
            signal: self.clone(),
            value: value.into(),
            spring: spring.into(),
            marker: PhantomData,
        }
    }
}
impl<W: 'static, C, T, M> AnimationChange<W, C>
    for SignalSpringChange<T, M>
where
    C: RegisterSignals,
    T: SpringValue<M> + Clone + ThreadSafe,
    W: SubjectSource<SignalId, T>,
{
    fn lower(
        self,
        sequence: &mut Sequence<'_, W, C>,
    ) -> TrackFragment {
        let id = sequence.context.signals().register(&self.signal);
        Property::new::<M>(id, path!(T))
            .spring_to::<M>(self.value, self.spring)
            .lower(sequence)
    }
}
