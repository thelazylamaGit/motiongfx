use crate::{
    BindingError, BindingPlan, BindingSource, Source, World,
    bindings::{BindingDependency, DerivedValues},
};
use std::time::Duration;

/// The scene's sample time as a source, from [`Scene::time`]. Reads the
/// time last passed to `sample` (clamped to the duration), so derived values
/// follow seeks in either direction.
///
/// [`Scene::time`]: crate::Scene::time
#[derive(Clone, Copy, Debug)]
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
