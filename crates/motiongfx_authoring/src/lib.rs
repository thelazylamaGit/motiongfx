//! Setup-only property descriptions that lower to ordinary MotionGfx actions.
//! There is no expression graph or additional playback evaluator.

use motiongfx::prelude::*;
use motiongfx::subject::SubjectId;
use std::ops::{Add, Mul};
use std::time::Duration;

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
        value: T,
    ) -> Change<I, S, T, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + ThreadSafe,
    {
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
pub struct Sequence<'a, W> {
    builder: TimelineBuilder<'a, W>,
    fragments: Vec<TrackFragment>,
    duration: Duration,
    ease: EaseFn,
}

impl<'a, W: 'static> Sequence<'a, W> {
    pub fn new(registry: &'a mut Registry) -> Self {
        let defaults = AnimationDefaults::default();
        Self {
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
    pub fn action<I, S, T, F>(
        &mut self,
        change: Change<I, S, T, F>,
    ) -> TrackFragment
    where
        I: SubjectId,
        S: Clone + ThreadSafe,
        T: Clone + ThreadSafe,
        F: Fn(&T) -> T + ThreadSafe,
        W: SubjectSource<I, S>,
    {
        self.builder
            .act_builder(
                change.property.id,
                change.property.path,
                change.target,
            )
            .with_interp(change.property.interp)
            .with_ease(change.ease.unwrap_or(self.ease))
            .play(change.duration.unwrap_or(self.duration))
    }

    pub fn play<I, S, T, F>(&mut self, change: Change<I, S, T, F>)
    where
        I: SubjectId,
        S: Clone + ThreadSafe,
        T: Clone + ThreadSafe,
        F: Fn(&T) -> T + ThreadSafe,
        W: SubjectSource<I, S>,
    {
        let fragment = self.action(change);
        self.append(fragment);
    }

    /// Accepts existing `.ord_all()` / `.ord_flow()` fragments as well.
    pub fn append(&mut self, fragment: TrackFragment) {
        self.fragments.push(fragment);
    }

    pub fn wait(&mut self, duration: Duration) {
        self.append(TrackFragment::silent(duration));
    }

    pub fn compile(self) -> Timeline<W> {
        self.builder.compile(self.fragments.ord_chain().compile())
    }
}
