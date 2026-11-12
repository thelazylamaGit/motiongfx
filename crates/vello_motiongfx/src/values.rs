//! Scene-local storage for standalone signals and an absolute-time source.
use crate::{Graphics, Scene, SignalId, Source};
use motiongfx::prelude::{SubjectSource, ThreadSafe};
use motiongfx_authoring::bindings::{
    BindingDependency, DerivedValues,
};
use motiongfx_authoring::{
    BindingError, BindingPlan, BindingSource, SignalValues,
    SignalWorld,
};
use std::time::Duration;

impl SignalWorld for Graphics {
    fn signals_mut(&mut self) -> &mut SignalValues {
        &mut self.values
    }
}
impl<T: ThreadSafe> SubjectSource<SignalId, T> for Graphics {
    fn get_source(&self, id: SignalId) -> Option<&T> {
        self.values.get(id)
    }
    fn apply_source<R>(
        &mut self,
        id: SignalId,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        self.values.apply(id, f)
    }
}
impl Scene {
    /// Absolute sample time, including backward seeks. Not elapsed frame time.
    pub fn time(&self) -> SampleTime {
        SampleTime
    }
}
#[derive(Clone, Copy)]
pub struct SampleTime;
impl BindingSource<Graphics> for SampleTime {
    type Value = Duration;
    fn read(
        &self,
        world: &Graphics,
        _: &DerivedValues,
    ) -> Option<Duration> {
        Some(world.sample_time)
    }
    fn dependencies(&self) -> Vec<BindingDependency> {
        Vec::new()
    }
    fn validate(&self, _: &Graphics) -> Result<(), BindingError> {
        Ok(())
    }
}

impl Source<Graphics> for SampleTime {
    type Value = Duration;
    type Prepared = Self;
    type Owned = Self;
    fn owned(&self) -> Self {
        *self
    }
    fn register(
        &self,
        _: &mut Graphics,
        _: &mut BindingPlan<Graphics>,
    ) -> Self {
        *self
    }
}
