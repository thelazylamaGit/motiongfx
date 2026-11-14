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
    SceneObject, SceneSequence, UpdateTargets, World,
};
pub use values::SampleTime;
mod signals;
mod source;
mod spring;
mod target;
pub use bindings::{BindingError, BindingPlan, BindingSource};
pub use signals::{
    RegisterSignals, Signal, SignalId, SignalValues, SignalWorld,
    signal,
};
pub use source::{Computed, Source, computed};
pub use spring::{
    Spring, SpringCurve, SpringError, SpringSample, SpringValue,
};
pub use target::{Field, Target};

/// Library-wide authoring defaults. Individual sequences and changes may
/// override these without changing other animations or process-global state.
#[derive(Clone, Copy)]
pub struct AnimationDefaults {
    pub duration: Duration,
    pub ease: EaseFn,
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
    interp: InterpFn<T>,
    /// Changes only at the end of an action (text, caps/joins), so it
    /// cannot spring.
    discrete: bool,
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
            interp: T::interp,
            discrete: false,
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
            interp: |a, b, progress| {
                if progress < 1.0 { a.clone() } else { b.clone() }
            },
            discrete: true,
        }
    }

    /// A change whose target is calculated from the value at its start.
    /// Runs once while baking, never per frame.
    #[track_caller]
    pub fn map_target<M, F>(self, target: F) -> Animation<M>
    where
        M: 'static,
        I: SubjectId,
        S: Clone + ThreadSafe,
        T: Clone + ThreadSafe,
        F: Fn(&T) -> T + Clone + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        animation::change(self, target)
    }

    /// Animate to `value`.
    #[track_caller]
    pub fn to<M>(self, value: impl Into<T>) -> Animation<M>
    where
        M: 'static,
        I: SubjectId,
        S: Clone + ThreadSafe,
        T: Clone + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        let value = value.into();
        self.map_target(move |_| value.clone())
    }

    /// Animate by adding `delta` to the value at the change's start.
    #[track_caller]
    pub fn offset<M, D>(self, delta: D) -> Animation<M>
    where
        M: 'static,
        I: SubjectId,
        S: Clone + ThreadSafe,
        T: Clone + ThreadSafe + Add<D, Output = T>,
        D: Copy + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        self.map_target(move |previous: &T| previous.clone() + delta)
    }

    /// Animate by multiplying the value at the change's start by `factor`.
    #[track_caller]
    pub fn multiply<M, D>(self, factor: D) -> Animation<M>
    where
        M: 'static,
        I: SubjectId,
        S: Clone + ThreadSafe,
        T: Clone + ThreadSafe + Mul<D, Output = T>,
        D: Copy + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        self.map_target(move |previous: &T| previous.clone() * factor)
    }

    /// Replace the value instantly at the point where this is scheduled.
    #[track_caller]
    pub fn set<M>(self, value: impl Into<T>) -> Animation<M>
    where
        M: 'static,
        I: SubjectId,
        S: Clone + ThreadSafe,
        T: Clone + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        self.to(value).over(Duration::ZERO)
    }

    /// Shorthand for `.to(value).spring(spring)`.
    #[track_caller]
    pub fn spring_to<M>(
        self,
        value: impl Into<T>,
        spring: impl Into<Spring>,
    ) -> Animation<M>
    where
        M: 'static,
        I: SubjectId,
        S: Clone + ThreadSafe,
        T: Clone + ThreadSafe,
        World<M>: SubjectSource<I, S>,
    {
        self.to(value).spring(spring)
    }
}

/// A thin collector for existing MotionGfx track fragments.
pub struct Sequence<'a, W, C = ()> {
    context: C,
    builder: TimelineBuilder<'a, W>,
    fragments: Vec<TrackFragment>,
    duration: Duration,
    ease: EaseFn,
}

impl<'a, W: 'static> Sequence<'a, W> {
    pub fn new(registry: &'a mut Registry) -> Self {
        Self::with_context(registry, ())
    }
}

impl<'a, W: 'static> Sequence<'a, W, &'a mut SignalValues> {
    pub fn with_signals(
        registry: &'a mut Registry,
        values: &'a mut SignalValues,
    ) -> Self {
        Self::with_context(registry, values)
    }
}

impl<'a, W: 'static, C> Sequence<'a, W, C> {
    pub(crate) fn with_context(
        registry: &'a mut Registry,
        context: C,
    ) -> Self {
        let defaults = AnimationDefaults::default();
        Self {
            context,
            builder: registry.create_builder::<W>(),
            fragments: Vec::new(),
            duration: defaults.duration,
            ease: defaults.ease,
        }
    }

    /// Construction context for model adapters lowering deferred targets.
    pub fn context_mut(&mut self) -> &mut C {
        &mut self.context
    }

    /// Defaults only affect subsequently scheduled changes.
    pub fn defaults(&mut self, duration: Duration, ease: EaseFn) {
        self.duration = duration;
        self.ease = ease;
    }

    /// Lower a property change without scheduling it, for custom track ordering.
    pub fn action<A: AnimationChange<W, C>>(
        &mut self,
        change: A,
    ) -> TrackFragment {
        change.lower(self)
    }

    pub fn play<A: AnimationChange<W, C>>(&mut self, change: A) {
        let fragment = self.action(change);
        self.append(fragment);
    }

    /// Start a collection of changes together, waiting for the longest one.
    /// Elements must have the same change type (for example, many x positions).
    /// For mixed property types, use `action` and existing track combinators.
    pub fn play_all<A: AnimationChange<W, C>>(
        &mut self,
        changes: impl IntoIterator<Item = A>,
    ) {
        let fragments: Vec<_> = changes
            .into_iter()
            .map(|change| self.action(change))
            .collect();
        self.append(fragments.ord_all());
    }

    /// Start successive children at fixed intervals between their starts.
    /// A child can be a change or a nested chain/parallel/flow block.
    /// Lowers directly to core `ord_flow`; returns the group's reserved
    /// duration (not its absolute end time). Empty blocks return zero.
    pub fn flow(
        &mut self,
        interval: Duration,
        author: impl FnOnce(&mut Self),
    ) -> Duration {
        self.group(author, |children| children.ord_flow(interval))
    }

    /// Accepts existing `.ord_all()` / `.ord_flow()` fragments as well.
    pub fn append(&mut self, fragment: TrackFragment) {
        self.fragments.push(fragment);
    }

    pub fn wait(&mut self, duration: Duration) {
        self.append(TrackFragment::silent(duration));
    }

    /// Start all actions authored inside this block at the same sequence time.
    /// Unlike `play_all`, a block can contain different subject/property types.
    pub fn parallel(&mut self, author: impl FnOnce(&mut Self)) {
        self.group(author, |children| children.ord_all());
    }

    /// Collect a sequential branch, useful inside a parallel block.
    pub fn chain(&mut self, author: impl FnOnce(&mut Self)) {
        self.group(author, |children| children.ord_chain());
    }

    // All scoped helpers collect the same typed fragments; core ordering
    // remains responsible for scheduling, including overlapping children.
    fn group(
        &mut self,
        author: impl FnOnce(&mut Self),
        order: impl FnOnce(Vec<TrackFragment>) -> TrackFragment,
    ) -> Duration {
        let start = self.fragments.len();
        author(self);
        let children: Vec<_> =
            self.fragments.drain(start..).collect();
        let fragment = order(children);
        let duration = fragment.duration();
        self.append(fragment);
        duration
    }

    /// Problems are reported through the scene context while lowering,
    /// so compiling the collected fragments cannot fail.
    pub fn compile(self) -> Timeline<W> {
        self.builder.compile(self.fragments.ord_chain().compile())
    }
}

/// Construction-time action description lowered to a core track fragment.
pub trait AnimationChange<W, C> {
    fn lower(
        self,
        sequence: &mut Sequence<'_, W, C>,
    ) -> TrackFragment;
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
