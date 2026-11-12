//! Standalone timeline parameters and an explicit sampled-time source.

use crate::{Graphics, Scene};
use motiongfx::prelude::{
    Interpolation, SubjectSource, ThreadSafe, path,
};
use motiongfx_authoring::Change;
use motiongfx_authoring::bindings::{
    BindingDependency, DerivedValues,
};
use motiongfx_authoring::{BindingError, BindingSource, Property};
use std::ops::{Add, Mul};
use std::time::Duration;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct SignalId(usize);

/// Writable, typed scene state. Storage does not require interpolation;
/// tween methods are available only when the value supports it.
pub struct Signal<T> {
    id: SignalId,
    marker: std::marker::PhantomData<fn() -> T>,
}
impl<T> Copy for Signal<T> {}
impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Clone + ThreadSafe> Signal<T> {
    pub fn to<M>(
        self,
        value: impl Into<T>,
    ) -> Change<SignalId, T, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Interpolation<M>,
    {
        Property::new(self.id, path!(T)).to(value)
    }
    pub fn set(
        self,
        value: impl Into<T>,
    ) -> Change<SignalId, T, T, impl Fn(&T) -> T + ThreadSafe> {
        Property::discrete(self.id, path!(T)).set(value)
    }
    pub fn offset<D, M>(
        self,
        delta: D,
    ) -> Change<SignalId, T, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Interpolation<M> + Add<D, Output = T>,
        D: Copy + ThreadSafe,
    {
        Property::new(self.id, path!(T)).offset(delta)
    }
    pub fn multiply<D, M>(
        self,
        factor: D,
    ) -> Change<SignalId, T, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Interpolation<M> + Mul<D, Output = T>,
        D: Copy + ThreadSafe,
    {
        Property::new(self.id, path!(T)).multiply(factor)
    }
}
impl<T: Clone + PartialEq + ThreadSafe> BindingSource<Graphics>
    for Signal<T>
{
    type Value = T;
    fn read(
        &self,
        world: &Graphics,
        values: &DerivedValues,
    ) -> Option<T> {
        Property::discrete(self.id, path!(T)).read(world, values)
    }
    fn dependencies(&self) -> Vec<BindingDependency> {
        <Property<SignalId, T, T> as BindingSource<Graphics>>::dependencies(&Property::discrete(self.id, path!(T)))
    }
    fn validate(&self, world: &Graphics) -> Result<(), BindingError> {
        Property::discrete(self.id, path!(T)).validate(world)
    }
}

impl Scene {
    /// Writable typed state with no drawable object or calculation node.
    /// Interpolation is required only by tween methods, not by storage or set.
    /// Like object properties, `set` describes an instantaneous timeline action
    /// and must be scheduled with `sequence.play(...)`.
    ///
    /// ```
    /// use motiongfx::prelude::s;
    /// use vello_motiongfx::{Circle, Scene};
    /// let mut scene = Scene::new();
    /// let visible = scene.signal(false);
    /// let opacity = scene.computed(visible, |v| if v { 1.0 } else { 0.0 });
    /// let dot = scene.add(Circle::new(10.0).opacity(opacity));
    /// let mut animation = scene.animate(|sequence| {
    ///     sequence.wait(s(1));
    ///     sequence.play(visible.set(true));
    /// });
    /// animation.sample(s(1));
    /// assert_eq!(animation.circle(dot).opacity, 1.0);
    /// ```
    ///
    /// Source types must match the destination:
    /// ```compile_fail
    /// use vello_motiongfx::{Circle, Point, Scene};
    /// let mut scene = Scene::new();
    /// let center = scene.signal(Point::ZERO);
    /// scene.add(Circle::new(10.0).opacity(center));
    /// ```
    pub fn signal<T>(&mut self, initial: T) -> Signal<T>
    where
        T: Clone + ThreadSafe,
    {
        let id = SignalId(self.graphics.values.len());
        self.graphics.values.push(Box::new(initial));
        Signal {
            id,
            marker: std::marker::PhantomData,
        }
    }

    /// Absolute sample time, including backward seeks. Not elapsed frame time.
    pub fn time(&self) -> SampleTime {
        SampleTime
    }
}

impl<T: ThreadSafe> SubjectSource<SignalId, T> for Graphics {
    fn get_source(&self, id: SignalId) -> Option<&T> {
        self.values.get(id.0)?.downcast_ref()
    }
    fn apply_source<R>(
        &mut self,
        id: SignalId,
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
