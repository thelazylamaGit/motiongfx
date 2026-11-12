//! Standalone timeline parameters and an explicit sampled-time source.

use crate::{Graphics, Scene};
use motiongfx::prelude::{
    Interpolation, SubjectSource, ThreadSafe, path,
};
use motiongfx_authoring::bindings::{
    BindingDependency, DerivedValues,
};
use motiongfx_authoring::{BindingError, BindingSource, Property};
use std::time::Duration;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct ValueId(usize);

impl Scene {
    /// A typed animation parameter, with no drawable object or dependency node.
    pub fn value<T, M>(
        &mut self,
        initial: T,
    ) -> Property<ValueId, T, T>
    where
        T: Clone + ThreadSafe + Interpolation<M>,
    {
        let id = ValueId(self.graphics.values.len());
        self.graphics.values.push(Box::new(initial));
        Property::new(id, path!(T))
    }

    /// Absolute sample time, including backward seeks. Not elapsed frame time.
    pub fn time(&self) -> SampleTime {
        SampleTime
    }
}

impl<T: ThreadSafe> SubjectSource<ValueId, T> for Graphics {
    fn get_source(&self, id: ValueId) -> Option<&T> {
        self.values.get(id.0)?.downcast_ref()
    }
    fn apply_source<R>(
        &mut self,
        id: ValueId,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        Some(f(self.values.get_mut(id.0)?.downcast_mut()?))
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
