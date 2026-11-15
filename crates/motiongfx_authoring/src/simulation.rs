//! [`Simulated`]: values that advance in fixed steps from their previous
//! state, run once when the scene compiles and then looked up by time.

use crate::bindings::{BindingDependency, DerivedValues};
use crate::{
    BindingError, BindingPlan, BindingSource, Computed, Source, World,
};
use motiongfx::prelude::ThreadSafe;
use std::any::Any;
use std::marker::PhantomData;
use std::time::Duration;

const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// The value of a simulation from [`Scene::simulate`] or
/// [`Scene::simulate_with`], for objects to follow.
///
/// Reads the state stored for the step at the scene's sample time, so it
/// follows seeks in either direction. Unlike a [`Signal`](crate::Signal),
/// it has no `.to(..)`: the simulation decides its value.
///
/// [`Scene::simulate`]: crate::Scene::simulate
/// [`Scene::simulate_with`]: crate::Scene::simulate_with
pub struct Simulated<T> {
    slot: usize,
    marker: PhantomData<fn() -> T>,
}
impl<T> Clone for Simulated<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Simulated<T> {}
impl<T> std::fmt::Debug for Simulated<T> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_tuple("Simulated").field(&self.slot).finish()
    }
}

impl<T: Clone + PartialEq + ThreadSafe> Simulated<T> {
    /// A value calculated from this simulation's state, for objects to
    /// follow.
    pub fn map<M: 'static, U, F>(
        &self,
        calculate: F,
    ) -> Computed<World<M>, U>
    where
        U: Clone + PartialEq + ThreadSafe,
        F: Fn(T) -> U + ThreadSafe,
    {
        Computed::new(*self, calculate)
    }
}

impl<M, T: Clone + PartialEq + ThreadSafe> BindingSource<World<M>>
    for Simulated<T>
{
    type Value = T;
    fn read(&self, world: &World<M>, _: &DerivedValues) -> Option<T> {
        world.simulations.state(*self, world.time()).cloned()
    }
    // Like the sample time, a simulation's value depends only on when the
    // scene is sampled, so it has nothing to order after.
    fn dependencies(&self) -> Vec<BindingDependency> {
        Vec::new()
    }
    fn validate(&self, world: &World<M>) -> Result<(), BindingError> {
        world
            .simulations
            .history::<T>(self.slot)
            .map(|_| ())
            .ok_or_else(|| {
                BindingError::MissingProperty(format!(
                    "simulation {}",
                    self.slot
                ))
            })
    }
}
impl<M, T: Clone + PartialEq + ThreadSafe> Source<World<M>>
    for Simulated<T>
{
    type Value = T;
    type Prepared = Self;
    type Owned = Self;
    fn owned(&self) -> Self {
        *self
    }
    fn register(
        &self,
        _: &mut World<M>,
        _: &mut BindingPlan<World<M>>,
    ) -> Self {
        *self
    }
}

/// Every simulation's stored states in one scene, one history per
/// simulation, indexed by step.
pub(crate) struct SimulationStates {
    rate: u32,
    histories: Vec<Box<dyn Any + Send + Sync>>,
}
impl Default for SimulationStates {
    fn default() -> Self {
        Self {
            rate: DEFAULT_RATE,
            histories: Vec::new(),
        }
    }
}
impl SimulationStates {
    /// Start a history holding only `initial`, returning its handle.
    pub(crate) fn add<T: Clone + PartialEq + ThreadSafe>(
        &mut self,
        initial: T,
    ) -> Simulated<T> {
        self.histories.push(Box::new(vec![initial]));
        Simulated {
            slot: self.histories.len() - 1,
            marker: PhantomData,
        }
    }
    fn history<T: 'static>(&self, slot: usize) -> Option<&Vec<T>> {
        self.histories.get(slot)?.downcast_ref()
    }
    fn history_mut<T: 'static>(
        &mut self,
        slot: usize,
    ) -> &mut Vec<T> {
        self.histories[slot].downcast_mut().expect(
            "a simulation's state type is fixed when it is added",
        )
    }
    /// The state stored for the step at `time`, or the last one past the
    /// end; `None` if `simulation` isn't from this scene.
    pub(crate) fn state<T: 'static>(
        &self,
        simulation: Simulated<T>,
        time: Duration,
    ) -> Option<&T> {
        let history = self.history::<T>(simulation.slot)?;
        history
            .get(step_index(time, self.rate).min(history.len() - 1))
    }
}

/// Steps per second when a scene doesn't set
/// [`Scene::simulation_rate`](crate::Scene::simulation_rate).
pub(crate) const DEFAULT_RATE: u32 = 60;

/// The step a sample at `time` shows: the last one started at or before it.
pub(crate) fn step_index(time: Duration, rate: u32) -> usize {
    (time.as_nanos() * u128::from(rate) / NANOS_PER_SECOND) as usize
}
/// When step `index` starts. Rounds up to whole nanoseconds, so
/// [`step_index`] of the result is `index` again even when a step isn't a
/// whole number of nanoseconds (1/60 s isn't).
pub(crate) fn step_time(index: usize, rate: u32) -> Duration {
    let nanos =
        (index as u128 * NANOS_PER_SECOND).div_ceil(u128::from(rate));
    Duration::from_nanos(nanos as u64)
}

/// One simulation's step function, with its inputs ready to read.
pub(crate) trait Stepper<M>: Send + Sync {
    /// Read the inputs, advance from the latest stored state by `dt`
    /// seconds, and store the result.
    fn step(
        &mut self,
        world: &mut World<M>,
        bindings: &BindingPlan<World<M>>,
        dt: f64,
    );
    /// Make room for `steps` more states.
    fn reserve(&self, world: &mut World<M>, steps: usize);
}

pub(crate) struct Simulation<T, R, F> {
    pub(crate) handle: Simulated<T>,
    /// Reads this step's inputs from the sampled scene.
    pub(crate) read: R,
    pub(crate) step: F,
}
impl<M, T, I, R, F> Stepper<M> for Simulation<T, R, F>
where
    T: Clone + PartialEq + ThreadSafe,
    R: Fn(&World<M>, &BindingPlan<World<M>>) -> I + Send + Sync,
    F: FnMut(&mut T, I, f64) + Send + Sync,
{
    fn step(
        &mut self,
        world: &mut World<M>,
        bindings: &BindingPlan<World<M>>,
        dt: f64,
    ) {
        let input = (self.read)(world, bindings);
        let history =
            world.simulations.history_mut::<T>(self.handle.slot);
        let mut next = history
            .last()
            .expect("a history starts with its initial state")
            .clone();
        (self.step)(&mut next, input, dt);
        history.push(next);
    }
    fn reserve(&self, world: &mut World<M>, steps: usize) {
        world
            .simulations
            .history_mut::<T>(self.handle.slot)
            .reserve_exact(steps);
    }
}

impl SimulationStates {
    /// Steps per second, read when sampling to find a time's step.
    pub(crate) fn set_rate(&mut self, rate: u32) {
        self.rate = rate;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_times_round_trip_at_rates_that_are_not_whole_nanoseconds()
    {
        for rate in [24, 30, 60, 90, 120, 144] {
            for index in 0..10_000 {
                assert_eq!(
                    step_index(step_time(index, rate), rate),
                    index,
                    "rate {rate}, step {index}"
                );
            }
        }
    }

    #[test]
    fn a_sample_between_steps_shows_the_earlier_step() {
        let just_before_second_step =
            step_time(2, 60) - Duration::from_nanos(1);
        assert_eq!(step_index(just_before_second_step, 60), 1);
    }
}
