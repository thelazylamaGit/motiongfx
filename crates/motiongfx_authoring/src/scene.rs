//! Renderer-independent construction and sampling over statically typed models.
use crate::{
    BindingError, BindingPlan, Property, SignalId, SignalValues,
    SignalWorld, Source,
};
use motiongfx::prelude::*;
use motiongfx::subject::SubjectId;
use std::{any::TypeId, time::Duration};

/// Checked finalization includes core action preparation and binding validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneError {
    Binding(BindingError),
    Preparation(crate::PreparationError),
    Target(String),
}
impl From<BindingError> for SceneError {
    fn from(e: BindingError) -> Self {
        Self::Binding(e)
    }
}
impl From<motiongfx::spring::SpringError> for SceneError {
    fn from(e: motiongfx::spring::SpringError) -> Self {
        Self::Preparation(e.into())
    }
}
impl From<crate::PreparationError> for SceneError {
    fn from(error: crate::PreparationError) -> Self {
        Self::Preparation(error)
    }
}
impl std::fmt::Display for SceneError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::Preparation(e) => e.fmt(f),
            Self::Target(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for SceneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Binding(e) => Some(e),
            Self::Preparation(e) => Some(e),
            Self::Target(_) => None,
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
    error: &'a mut Option<String>,
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
    pub fn reject(&mut self, error: impl Into<String>) {
        self.error.get_or_insert_with(|| error.into());
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
        }
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
    /// Compile the existing core timeline and initialize time zero. Panics on
    /// invalid bindings; use `try_animate` to handle preparation errors.
    #[track_caller]
    pub fn animate(
        self,
        author: impl FnOnce(&mut SceneSequence<'_, M>),
    ) -> Animation<M> {
        self.try_animate(author).expect("invalid scene preparation")
    }
    /// Finalize construction hooks, validate bindings, bake action targets, then
    /// sample time zero. Rendering and render cache allocation happen separately.
    pub fn try_animate(
        mut self,
        author: impl FnOnce(&mut SceneSequence<'_, M>),
    ) -> Result<Animation<M>, SceneError> {
        for (_, prepare) in std::mem::take(&mut self.preparation) {
            prepare(&mut self);
        }
        let mut registry = Registry::new();
        let mut target_error = None;
        let mut sequence = crate::Sequence::with_context(
            &mut registry,
            SceneContext {
                world: &mut self.world,
                error: &mut target_error,
                caches: Default::default(),
            },
        );
        author(&mut sequence);
        let mut timeline = sequence.try_compile()?;
        if let Some(error) = target_error {
            return Err(SceneError::Target(error));
        }
        self.bindings.prepare(&self.world, &timeline)?;
        timeline.bake_actions(&registry, &self.world);
        let mut animation = Animation {
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
pub struct Animation<M> {
    world: World<M>,
    registry: Registry,
    timeline: Timeline<World<M>>,
    bindings: BindingPlan<World<M>>,
    updates: Vec<NativeUpdate<M>>,
}
impl<M: 'static> Animation<M> {
    pub fn state(&self) -> &World<M> {
        &self.world
    }
    pub fn model(&self) -> &M {
        self.world.model()
    }
    pub fn duration(&self) -> Duration {
        self.timeline.tracks()[0].duration()
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
