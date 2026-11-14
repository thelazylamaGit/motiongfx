//! Renderer-independent construction and sampling over statically typed models.
use crate::{
    BindingError, BindingPlan, Property, SignalId, SignalValues,
    SignalWorld, Source,
};
use motiongfx::prelude::*;
use motiongfx::subject::SubjectId;
use std::{any::TypeId, time::Duration};

/// Problems found while compiling a scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneError {
    /// An invalid binding (cycle, missing property, conflicting writers).
    Binding(BindingError),
    /// Every problem found while compiling animations, each with the
    /// source location of the animation that caused it.
    Animation(Vec<crate::AnimationError>),
}
impl From<BindingError> for SceneError {
    fn from(e: BindingError) -> Self {
        Self::Binding(e)
    }
}
impl std::fmt::Display for SceneError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::Animation(errors) => {
                for (index, error) in errors.iter().enumerate() {
                    if index > 0 {
                        f.write_str("\n")?;
                    }
                    error.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}
impl std::error::Error for SceneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Binding(e) => Some(e),
            Self::Animation(_) => None,
        }
    }
}

/// Common reactive state and an application-selected model (or composite model).
/// Models implement typed SubjectSource access for this wrapper using their own
/// IDs. No per-sample type registry is used to select model storage.
pub struct World<M> {
    model: M,
    signals: SignalValues,
    time: Duration,
}
impl<M> World<M> {
    pub fn model(&self) -> &M {
        &self.model
    }
    pub fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }
    pub fn time(&self) -> Duration {
        self.time
    }
    /// Inspect a root registered by this scene. This lookup is for inspection, not binding evaluation.
    pub fn signal_value<T: ThreadSafe>(
        &self,
        signal: &crate::Signal<T>,
    ) -> Option<&T> {
        self.signals.registered_value(signal)
    }
}
impl<M> SignalWorld for World<M> {
    fn signals_mut(&mut self) -> &mut SignalValues {
        &mut self.signals
    }
}
impl<M, T: ThreadSafe> SubjectSource<SignalId, T> for World<M> {
    fn get_source(&self, id: SignalId) -> Option<&T> {
        self.signals.get(id)
    }
    fn apply_source<R>(
        &mut self,
        id: SignalId,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        self.signals.apply(id, f)
    }
}

pub type SceneSequence<'a, M> =
    crate::Sequence<'a, World<M>, SceneContext<'a, M>>;

/// Construction-only access for model-specific deferred animation targets.
/// The context and its memoized registration data are dropped before playback.
pub struct SceneContext<'a, M> {
    world: &'a mut World<M>,
    animation_errors: &'a mut Vec<crate::AnimationError>,
    /// Marker names seen so far, with where each was first used.
    markers: std::collections::HashMap<
        motiongfx::track::MarkerName,
        &'static std::panic::Location<'static>,
    >,
    caches: std::collections::HashMap<TypeId, Box<dyn std::any::Any>>,
}
impl<M> SceneContext<'_, M> {
    pub fn model(&self) -> &M {
        self.world.model()
    }
    pub fn model_mut(&mut self) -> &mut M {
        self.world.model_mut()
    }
    /// Memoize target resolution during construction only, never sampling.
    pub fn cache<T: Default + 'static>(&mut self) -> &mut T {
        self.caches
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut()
            .expect("cache key and value type agree")
    }
    /// Record a problem with one animation; `compile` reports them all.
    pub fn report(&mut self, error: crate::AnimationError) {
        self.animation_errors.push(error);
    }
    /// Claim a marker name. Returns `false` (and reports the duplicate)
    /// if the name was already used.
    pub(crate) fn register_marker(
        &mut self,
        name: &motiongfx::track::MarkerName,
        location: &'static std::panic::Location<'static>,
    ) -> bool {
        if let Some(&first) = self.markers.get(name) {
            self.report(crate::AnimationError {
                location,
                kind: crate::AnimationErrorKind::DuplicateMarker {
                    name: name.clone(),
                    first,
                },
            });
            return false;
        }
        self.markers.insert(name.clone(), location);
        true
    }
}
impl<M> crate::RegisterSignals for SceneContext<'_, M> {
    fn signals(&mut self) -> &mut SignalValues {
        &mut self.world.signals
    }
}
type Prepare<M> = Box<dyn FnOnce(&mut Scene<M>) + Send + Sync>;
type Update<M> =
    dyn FnMut(&mut World<M>, &BindingPlan<World<M>>) + Send + Sync;

/// A prepared native callback. Model adapters select/borrow their typed targets.
pub struct NativeUpdate<M>(Box<Update<M>>);
impl<M> NativeUpdate<M> {
    pub fn new(
        update: impl FnMut(&mut World<M>, &BindingPlan<World<M>>)
        + ThreadSafe,
    ) -> Self {
        Self(Box::new(update))
    }
}
/// Model adapter for borrowing callback targets. Preparation checks target
/// existence/aliasing; the returned callback reads sources before mutable
/// borrowing. No update dependency sorting or writer validation is implied.
pub trait UpdateTargets<M>: Sized {
    type Objects<'a>;
    fn callback<S, F>(
        self,
        source: S,
        update: F,
        world: &World<M>,
    ) -> NativeUpdate<M>
    where
        S: crate::BindingSource<World<M>>,
        F: for<'a> FnMut(Self::Objects<'a>, S::Value) + ThreadSafe;
}
/// Register an object or reusable component in the shared scene and return its
/// typed animation handles. Components may add several constituent objects.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be added to the scene",
    label = "not a scene object",
    note = "pass an object description such as `Circle::new(..)`, or implement `SceneObject` for a component"
)]
pub trait SceneObject<M> {
    type Handle;
    fn insert_into(self, scene: &mut Scene<M>) -> Self::Handle;
}

/// One authoring lifecycle for any model, including composite 2D/3D stores.
pub struct Scene<M> {
    world: World<M>,
    bindings: BindingPlan<World<M>>,
    updates: Vec<NativeUpdate<M>>,
    preparation: Vec<(TypeId, Prepare<M>)>,
    defaults: crate::AnimationDefaults,
}
impl<M: Default + 'static> Default for Scene<M> {
    fn default() -> Self {
        Self::with_model(M::default())
    }
}
impl<M: Default + 'static> Scene<M> {
    pub fn new() -> Self {
        Self::default()
    }
}
impl<M: 'static> Scene<M> {
    pub fn with_model(model: M) -> Self {
        Self {
            world: World {
                model,
                signals: SignalValues::default(),
                time: Duration::ZERO,
            },
            bindings: BindingPlan::default(),
            updates: Vec::new(),
            preparation: Vec::new(),
            defaults: crate::AnimationDefaults::default(),
        }
    }
    /// Duration and ease for every animation that does not set its own.
    /// Read when the scene compiles, so it applies to all animations,
    /// including ones built before this call.
    pub fn defaults(&mut self, duration: Duration, ease: EaseFn) {
        self.defaults = crate::AnimationDefaults { duration, ease };
    }
    pub fn model(&self) -> &M {
        self.world.model()
    }
    pub fn model_mut(&mut self) -> &mut M {
        self.world.model_mut()
    }
    pub fn add<T: SceneObject<M>>(&mut self, object: T) -> T::Handle {
        object.insert_into(self)
    }
    pub fn time(&self) -> crate::SampleTime {
        crate::SampleTime
    }
    /// A keyed construction hook. Reconfiguring the same model feature replaces
    /// its previous hook. These type identities are never looked up in sampling.
    pub fn prepare_with<K: 'static>(
        &mut self,
        prepare: impl FnOnce(&mut Self) + ThreadSafe,
    ) {
        let key = TypeId::of::<K>();
        self.preparation.retain(|(previous, _)| *previous != key);
        self.preparation.push((key, Box::new(prepare)));
    }
    /// Explicit cached property wiring. Prefer source-aware model builders for
    /// ordinary relationships; both register into this same dependency plan.
    pub fn bind<I, S, T, A, F>(
        &mut self,
        destination: impl Into<Property<I, S, T>>,
        source: A,
        calculate: F,
    ) where
        I: SubjectId,
        S: 'static,
        T: Clone + ThreadSafe,
        World<M>: SubjectSource<I, S>,
        A: Source<World<M>>,
        F: Fn(A::Value) -> T + ThreadSafe,
    {
        let source =
            source.register(&mut self.world, &mut self.bindings);
        self.bindings.bind(destination, source, calculate);
    }
    /// A native callback on every sample, after all bindings and in registration
    /// order. Assign from sampled inputs for seeking; accumulating frame deltas
    /// does not reconstruct earlier states. Invalid sources/targets panic here.
    pub fn update<T, S, F>(
        &mut self,
        targets: T,
        source: S,
        update: F,
    ) where
        T: UpdateTargets<M>,
        S: Source<World<M>>,
        F: for<'a> FnMut(T::Objects<'a>, S::Value) + ThreadSafe,
    {
        let source =
            source.register(&mut self.world, &mut self.bindings);
        self.bindings
            .validate_source(&self.world, &source)
            .expect("invalid update source");
        self.updates.push(targets.callback(
            source,
            update,
            &self.world,
        ));
    }
    /// Compile animations into a playable [`CompiledScene`]. The steps run in
    /// order, like [`chain`](crate::chain). Panics with every problem found
    /// (each with file and line); use [`try_compile`](Self::try_compile)
    /// to handle them instead.
    #[track_caller]
    pub fn compile(
        self,
        steps: impl IntoIterator<Item = crate::Animation<M>>,
    ) -> CompiledScene<M> {
        self.try_compile(steps).unwrap_or_else(|error| {
            panic!("invalid scene preparation:\n{error}")
        })
    }
    /// Run preparation hooks, lower the animations in source order,
    /// validate bindings, bake action targets, then sample time zero.
    pub fn try_compile(
        self,
        steps: impl IntoIterator<Item = crate::Animation<M>>,
    ) -> Result<CompiledScene<M>, SceneError> {
        // Lower each step as it is read; the list is never copied.
        self.try_animate(|sequence| {
            let fragment = steps
                .into_iter()
                .map(|step| step.lower(sequence))
                .ord_chain();
            sequence.append(fragment);
        })
    }
    /// Compile the existing core timeline and initialize time zero. Panics on
    /// invalid bindings; use `try_animate` to handle preparation errors.
    #[track_caller]
    pub fn animate(
        self,
        author: impl FnOnce(&mut SceneSequence<'_, M>),
    ) -> CompiledScene<M> {
        self.try_animate(author).expect("invalid scene preparation")
    }
    /// Finalize construction hooks, validate bindings, bake action targets, then
    /// sample time zero. Rendering and render cache allocation happen separately.
    pub fn try_animate(
        mut self,
        author: impl FnOnce(&mut SceneSequence<'_, M>),
    ) -> Result<CompiledScene<M>, SceneError> {
        for (_, prepare) in std::mem::take(&mut self.preparation) {
            prepare(&mut self);
        }
        let mut registry = Registry::new();
        let mut animation_errors = Vec::new();
        let mut sequence = crate::Sequence::with_context(
            &mut registry,
            SceneContext {
                world: &mut self.world,
                animation_errors: &mut animation_errors,
                markers: Default::default(),
                caches: Default::default(),
            },
        );
        sequence.defaults(self.defaults.duration, self.defaults.ease);
        author(&mut sequence);
        let mut timeline = sequence.compile();
        if !animation_errors.is_empty() {
            return Err(SceneError::Animation(animation_errors));
        }
        self.bindings.prepare(&self.world, &timeline)?;
        timeline.bake_actions(&registry, &self.world);
        let mut animation = CompiledScene {
            world: self.world,
            registry,
            timeline,
            bindings: self.bindings,
            updates: self.updates,
        };
        animation.sample(Duration::ZERO);
        Ok(animation)
    }
}
/// One compiled core timeline, native dependency plan and sampled model.
/// Renderers consume the read-only model; this type owns no rendering resources.
pub struct CompiledScene<M> {
    world: World<M>,
    registry: Registry,
    timeline: Timeline<World<M>>,
    bindings: BindingPlan<World<M>>,
    updates: Vec<NativeUpdate<M>>,
}
impl<M: 'static> CompiledScene<M> {
    pub fn state(&self) -> &World<M> {
        &self.world
    }
    pub fn model(&self) -> &M {
        self.world.model()
    }
    pub fn duration(&self) -> Duration {
        self.timeline.tracks()[0].duration()
    }
    /// The time of the marker called `name`, if the animation has one.
    pub fn marker(&self, name: &str) -> Option<Duration> {
        self.timeline.tracks()[0].marker(name)
    }
    /// Every marker with its time, sorted by time.
    pub fn markers(
        &self,
    ) -> &[(motiongfx::track::MarkerName, Duration)] {
        self.timeline.tracks()[0].markers()
    }
    /// Reconstruct the core timeline at clamped absolute time, evaluate cached
    /// bindings, then run native callbacks. Baking does not precompute frames.
    pub fn sample(&mut self, time: Duration) {
        self.timeline.set_target_time(time);
        self.world.time = self.timeline.target_time();
        self.timeline.queue_actions();
        self.timeline
            .sample_queued_actions(&self.registry, &mut self.world);
        self.bindings.evaluate(&mut self.world);
        for update in &mut self.updates {
            (update.0)(&mut self.world, &self.bindings);
        }
    }
}
