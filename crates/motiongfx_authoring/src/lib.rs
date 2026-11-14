//! Renderer-independent scene authoring over ordinary MotionGfx timelines.
//! Timeline descriptions lower to core actions. Optional native bindings
//! evaluate explicit property relationships after timeline sampling.

use motiongfx::prelude::*;
use motiongfx::subject::SubjectId;
use std::ops::{Add, Mul};
use std::time::Duration;

mod animation;
pub mod bindings;
mod input;
pub use animation::{
    Animation, AnimationError, AnimationErrorKind, Step, Timing, all,
    chain, flow, lower_change, marker, wait,
};
mod macros;
mod object;
mod scene;
pub use input::{Input, PreparedInput};
pub use object::{ObjectDefinition, Register};
mod values;
pub use scene::{
    CompiledScene, NativeUpdate, Scene, SceneContext, SceneError,
    SceneObject, UpdateTargets, World,
};
pub use values::SampleTime;
mod signals;
mod source;
mod spring;
mod target;
pub use bindings::{BindingError, BindingPlan, BindingSource};
pub use signals::{
    Signal, SignalId, SignalValues, SignalWorld, signal,
};
pub use source::{Computed, Source, computed};
pub use spring::{
    Spring, SpringCurve, SpringError, SpringSample, SpringValue,
};
pub use target::{Field, Target};

/// Duration and ease for changes that don't set their own; see
/// [`Scene::defaults`].
#[derive(Clone, Copy)]
pub(crate) struct AnimationDefaults {
    pub(crate) duration: Duration,
    pub(crate) ease: EaseFn,
}

impl Default for AnimationDefaults {
    fn default() -> Self {
        Self {
            duration: s(1),
            ease: smooth,
        }
    }
}

/// Normalized sigmoid easing (Manim's default `smooth` function).
pub fn smooth(t: f32) -> f32 {
    let error = 1.0 / (1.0 + 5.0_f32.exp());
    let sigmoid = 1.0 / (1.0 + (-10.0 * (t - 0.5)).exp());
    ((sigmoid - error) / (1.0 - 2.0 * error)).clamp(0.0, 1.0)
}

/// A subject identity plus a statically typed field accessor.
pub struct Property<I, S, T> {
    id: I,
    path: Path<S, T>,
    interp: Interp<T>,
}

/// How a property moves from its start value to its target.
pub(crate) enum Interp<T> {
    /// Along the value type's interpolation. Can spring.
    Smooth(InterpFn<T>),
    /// Jumps to the target at the end of the change (text, caps/joins),
    /// so it cannot spring.
    Discrete,
}
impl<T> Copy for Interp<T> {}
impl<T> Clone for Interp<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Clone> Interp<T> {
    pub(crate) fn function(self) -> InterpFn<T> {
        match self {
            Self::Smooth(interp) => interp,
            Self::Discrete => jump,
        }
    }
}
fn jump<T: Clone>(start: &T, end: &T, progress: f32) -> T {
    if progress < 1.0 {
        start.clone()
    } else {
        end.clone()
    }
}

/// Shows the object ID and whether the property is discrete.
impl<I: std::fmt::Debug, S, T> std::fmt::Debug for Property<I, S, T> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let discrete = matches!(self.interp, Interp::Discrete);
        f.debug_struct("Property")
            .field("id", &self.id)
            .field("discrete", &discrete)
            .finish_non_exhaustive()
    }
}
impl<I: Copy, S, T> Copy for Property<I, S, T> {}
impl<I: Copy, S, T> Clone for Property<I, S, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I: Copy, S, T> Property<I, S, T> {
    pub fn new<M>(id: I, path: Path<S, T>) -> Self
    where
        T: Interpolation<M>,
    {
        Self {
            id,
            path,
            interp: Interp::Smooth(T::interp),
        }
    }

    /// A property whose value changes only at the end of an action.
    /// Useful for text and other values without a meaningful interpolation.
    pub fn discrete(id: I, path: Path<S, T>) -> Self
    where
        T: Clone,
    {
        Self {
            id,
            path,
            interp: Interp::Discrete,
        }
    }
}

/// Changes. `M` is the scene's model and is inferred from where the
/// animation is used.
impl<I, S, T> Property<I, S, T>
where
    I: SubjectId,
    S: Clone + ThreadSafe,
    T: Clone + ThreadSafe,
{
    /// Animate to a target calculated from the value at the change's
    /// start. Runs once while compiling, never per frame.
    #[track_caller]
    pub fn to_with<M: 'static>(
        self,
        target: impl Fn(&T) -> T + Clone + ThreadSafe,
    ) -> Animation<M>
    where
        World<M>: SubjectSource<I, S>,
    {
        animation::change(self, target)
    }

    /// Animate to `value`.
    #[track_caller]
    pub fn to<M: 'static>(self, value: impl Into<T>) -> Animation<M>
    where
        World<M>: SubjectSource<I, S>,
    {
        let value = value.into();
        self.to_with(move |_| value.clone())
    }

    /// Replace the value instantly at the point where this is scheduled.
    #[track_caller]
    pub fn set<M: 'static>(self, value: impl Into<T>) -> Animation<M>
    where
        World<M>: SubjectSource<I, S>,
    {
        self.to(value).over(Duration::ZERO)
    }

    /// Animate by adding `delta` to the value at the change's start.
    #[track_caller]
    pub fn offset<M: 'static, D>(self, delta: D) -> Animation<M>
    where
        T: Add<D, Output = T>,
        D: Copy + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        self.to_with(move |previous: &T| previous.clone() + delta)
    }

    /// Animate by multiplying the value at the change's start by `factor`.
    #[track_caller]
    pub fn multiply<M: 'static, D>(self, factor: D) -> Animation<M>
    where
        T: Mul<D, Output = T>,
        D: Copy + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        self.to_with(move |previous: &T| previous.clone() * factor)
    }
}

impl<I, S, T> Property<I, S, T> {
    /// Describe a calculation from this sampled property.
    pub fn map<W: 'static, U, F>(
        &self,
        calculate: F,
    ) -> Computed<W, U>
    where
        Self: Source<W>,
        U: Clone + PartialEq + ThreadSafe,
        F: Fn(<Self as Source<W>>::Value) -> U + ThreadSafe,
    {
        Computed::new(self, calculate)
    }
}

mod updates;
pub use updates::{UpdateHandle, UpdateStorage};
