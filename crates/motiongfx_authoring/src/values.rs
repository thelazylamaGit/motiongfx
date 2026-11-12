use crate::{
    BindingError, BindingPlan, BindingSource, Source, World,
    bindings::{BindingDependency, DerivedValues},
};
use std::time::Duration;

/// Absolute clamped timeline time, including backward seeks.
#[derive(Clone, Copy)]
pub struct SampleTime;
impl<M> BindingSource<World<M>> for SampleTime {
    type Value = Duration;
    fn read(
        &self,
        world: &World<M>,
        _: &DerivedValues,
    ) -> Option<Duration> {
        Some(world.time())
    }
    fn dependencies(&self) -> Vec<BindingDependency> {
        Vec::new()
    }
    fn validate(&self, _: &World<M>) -> Result<(), BindingError> {
        Ok(())
    }
}
impl<M> Source<World<M>> for SampleTime {
    type Value = Duration;
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
