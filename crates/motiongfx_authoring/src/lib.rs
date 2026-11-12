//! Renderer-independent scene authoring over ordinary MotionGfx timelines.
//! Timeline descriptions lower to core actions. Optional native bindings
//! evaluate explicit property relationships after timeline sampling.

use motiongfx::prelude::*;
use motiongfx::subject::SubjectId;
use std::ops::{Add, Mul};
use std::time::Duration;

pub mod bindings;
mod input;
mod macros;
mod object;
mod scene;
pub use input::{Input, PreparedInput};
pub use object::{ObjectDefinition, Register};
mod values;
pub use scene::{
    Animation, NativeUpdate, Scene, SceneObject, SceneSequence,
    UpdateTargets, World,
};
pub use values::SampleTime;
mod signals;
mod source;
pub use bindings::{BindingError, BindingPlan, BindingSource};
pub use signals::{
    RegisterSignals, Signal, SignalChange, SignalId, SignalValues,
    SignalWorld, signal,
};
pub use source::{Computed, Source, computed};

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
        }
    }

    /// A custom action target, resolved during MotionGfx baking, not each frame.
    pub fn map_target<F>(self, target: F) -> Change<I, S, T, F>
    where
        F: Fn(&T) -> T + ThreadSafe,
    {
        Change {
            property: self,
            target,
            duration: None,
            ease: None,
        }
    }

    pub fn to(
        self,
        value: impl Into<T>,
    ) -> Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + ThreadSafe,
    {
        let value = value.into();
        self.map_target(move |_| value.clone())
    }

    pub fn offset<D>(
        self,
        delta: D,
    ) -> Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + Add<D, Output = T>,
        D: Copy + ThreadSafe,
    {
        self.map_target(move |previous| previous.clone() + delta)
    }

    /// Describe an immediate change at the point where it is scheduled.
    /// Use `sequence.play(property.set(value))`; this does not mutate state
    /// merely by constructing the description.
    pub fn set(
        self,
        value: impl Into<T>,
    ) -> Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + ThreadSafe,
    {
        self.to(value).over(Duration::ZERO)
    }

    pub fn multiply<D>(
        self,
        factor: D,
    ) -> Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + Mul<D, Output = T>,
        D: Copy + ThreadSafe,
    {
        self.map_target(move |previous| previous.clone() * factor)
    }
}

/// Consumed when scheduled; does not survive as an extra runtime layer.
#[must_use = "Schedule the change with sequence.play(...) or sequence.play_all(...)"]
pub struct Change<I, S, T, F> {
    property: Property<I, S, T>,
    target: F,
    duration: Option<Duration>,
    ease: Option<EaseFn>,
}

impl<I, S, T, F> Change<I, S, T, F> {
    pub fn over(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
    pub fn ease(mut self, ease: EaseFn) -> Self {
        self.ease = Some(ease);
        self
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
    fn with_context(registry: &'a mut Registry, context: C) -> Self {
        let defaults = AnimationDefaults::default();
        Self {
            context,
            builder: registry.create_builder::<W>(),
            fragments: Vec::new(),
            duration: defaults.duration,
            ease: defaults.ease,
        }
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

    /// Start successive changes at fixed intervals, waiting for the last end.
    /// The interval is measured between starts, not after each change finishes.
    pub fn stagger<A: AnimationChange<W, C>>(
        &mut self,
        interval: Duration,
        changes: impl IntoIterator<Item = A>,
    ) {
        let fragments: Vec<_> = changes
            .into_iter()
            .map(|change| self.action(change))
            .collect();
        self.append(fragments.ord_flow(interval));
    }

    /// Start successive children at fixed intervals between their starts.
    /// A child can be a change or a nested chain/parallel/staggered block.
    /// Lowers directly to core `ord_flow`; returns the group's reserved
    /// duration (not its absolute end time). Empty blocks return zero.
    pub fn staggered(
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

    /// Set a property at the current sequence time without consuming time.
    pub fn set<I, S, T>(
        &mut self,
        property: Property<I, S, T>,
        value: impl Into<T>,
    ) where
        I: SubjectId,
        S: Clone + ThreadSafe,
        T: Clone + ThreadSafe,
        W: SubjectSource<I, S>,
    {
        self.play(property.set(value));
    }

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
impl<W: 'static, C, I, S, T, F> AnimationChange<W, C>
    for Change<I, S, T, F>
where
    I: SubjectId,
    S: Clone + ThreadSafe,
    T: Clone + ThreadSafe,
    F: Fn(&T) -> T + ThreadSafe,
    W: SubjectSource<I, S>,
{
    fn lower(
        self,
        sequence: &mut Sequence<'_, W, C>,
    ) -> TrackFragment {
        sequence
            .builder
            .act_builder(
                self.property.id,
                self.property.path,
                self.target,
            )
            .with_interp(self.property.interp)
            .with_ease(self.ease.unwrap_or(sequence.ease))
            .play(self.duration.unwrap_or(sequence.duration))
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
