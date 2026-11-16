//! [`Scene`] (building) and [`CompiledScene`] (playing), shared by every
//! model, plus the [`World`] they store objects and signals in.
use crate::simulation::{
    DEFAULT_RATE, Simulated, Simulation, SimulationStates, Stepper,
    step_index, step_time,
};
use crate::{
    BindingError, BindingPlan, Property, SignalId, SignalValues,
    SignalWorld, Source,
};
use motiongfx::action::ActionId;
use motiongfx::prelude::*;
use motiongfx::subject::SubjectId;
use motiongfx::track::CueId;
use std::{
    any::{Any, TypeId},
    sync::Arc,
    time::Duration,
};

/// Problems found while compiling a scene.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum CompileError {
    /// An invalid binding, such as a cycle or two bindings writing one field.
    Binding(BindingError),
    /// Every problem found in the animations, each with the file and line
    /// of the animation that caused it.
    Animation(Vec<crate::AnimationError>),
}
impl From<BindingError> for CompileError {
    fn from(e: BindingError) -> Self {
        Self::Binding(e)
    }
}
impl std::fmt::Display for CompileError {
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
impl std::error::Error for CompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Displays its inner error, so it forwards that error's source.
        match self {
            Self::Binding(e) => e.source(),
            Self::Animation(_) => None,
        }
    }
}

/// Everything a scene animates: the model `M` (its objects), the current
/// signal values and the sample time.
///
/// A model makes its objects animatable by implementing [`WorldObject`]
/// for each object type, with its own ID types.
pub struct World<M> {
    model: M,
    signals: SignalValues,
    pub(crate) simulations: SimulationStates,
    time: Duration,
}
impl<M> World<M> {
    /// The model: the scene's objects.
    pub fn model(&self) -> &M {
        &self.model
    }
    /// The model, mutably.
    pub fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }
    /// The time last sampled.
    pub fn time(&self) -> Duration {
        self.time
    }
    /// The current value of `signal`, or `None` if this scene never used it.
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
/// An object type stored in a [`World<M>`] under IDs of type `I`, such as
/// a 2D model's circles under `CircleId`. Implementing it makes the type
/// animatable in every world with that model.
///
/// Implement this rather than core's `SubjectSource<I, Self>` for
/// `World<M>`: a blanket impl provides that, and because this is a bound
/// on the object type, traits such as [`ObjectHandle`] can require it. A
/// function generic over handles can then animate their fields without
/// its own `where World<M>: SubjectSource<..>` clause.
pub trait WorldObject<I, M>: Sized + 'static {
    /// The object `id`, if `world` has it.
    fn get(world: &World<M>, id: I) -> Option<&Self>;
    /// The object `id`, mutably, if `world` has it.
    fn get_mut(world: &mut World<M>, id: I) -> Option<&mut Self>;
}
impl<M, I: SubjectId, S: WorldObject<I, M>> SubjectSource<I, S>
    for World<M>
{
    fn get_source(&self, id: I) -> Option<&S> {
        S::get(self, id)
    }
    fn apply_source<R>(
        &mut self,
        id: I,
        f: impl FnOnce(&mut S) -> R,
    ) -> Option<R> {
        S::get_mut(self, id).map(f)
    }
}
impl<M, T: ThreadSafe> WorldObject<SignalId, M> for T {
    fn get(world: &World<M>, id: SignalId) -> Option<&T> {
        world.signals.get(id)
    }
    fn get_mut(world: &mut World<M>, id: SignalId) -> Option<&mut T> {
        world.signals.get_mut(id)
    }
}

/// A handle to an object in a scene, as returned by `scene.add`: it names
/// the object's ID and the type of its state, so
/// [`CompiledScene::get`] can return a `&Circle` for a circle's handle.
/// Model crates build traits for shared fields, such as a position, on it.
pub trait ObjectHandle<M>: Copy {
    /// The object's ID type, such as `CircleId`.
    type Id: SubjectId;
    /// The object's state, such as `Circle`.
    type State: WorldObject<Self::Id, M> + Clone + ThreadSafe;
    /// The object's ID.
    fn id(self) -> Self::Id;
}

/// What a [`Step`](crate::Step) can use while the scene compiles: the
/// model, caches and error reporting. Steps add their changes to the
/// timeline with [`lower_change`](crate::lower_change).
pub struct CompileContext<'a, M> {
    world: &'a mut World<M>,
    pub(crate) builder: TimelineBuilder<'a, World<M>>,
    pub(crate) defaults: crate::AnimationDefaults,
    errors: Vec<crate::AnimationError>,
    /// Each core action with the line that created it, for overlap errors.
    pub(crate) actions:
        Vec<(ActionId, &'static std::panic::Location<'static>)>,
    /// Marker names seen so far, with where each was first used.
    markers: std::collections::HashMap<
        crate::MarkerName,
        &'static std::panic::Location<'static>,
    >,
    /// What each core cue stands for, indexed by its `CueId`.
    cued: Vec<Cued>,
    caches: std::collections::HashMap<TypeId, Box<dyn std::any::Any>>,
}
impl<M> CompileContext<'_, M> {
    /// The model being compiled.
    pub fn model(&self) -> &M {
        self.world.model()
    }
    /// The model, mutably; for example to register a text selection.
    pub fn model_mut(&mut self) -> &mut M {
        self.world.model_mut()
    }
    /// A cache of type `T`, shared by every step while this scene compiles;
    /// for example to resolve each text selection once.
    pub fn cache<T: Default + 'static>(&mut self) -> &mut T {
        self.caches
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut()
            .expect("cache key and value type agree")
    }
    /// Record a problem with one animation. `compile` reports them all
    /// together.
    pub fn report(&mut self, error: crate::AnimationError) {
        self.errors.push(error);
    }
    pub(crate) fn signals(&mut self) -> &mut SignalValues {
        &mut self.world.signals
    }
    /// Claim a marker name and a cue for it. Returns `None` (and reports
    /// the duplicate) if the name was already used.
    pub(crate) fn register_marker(
        &mut self,
        name: &crate::MarkerName,
        location: &'static std::panic::Location<'static>,
    ) -> Option<CueId> {
        if let Some(&first) = self.markers.get(name) {
            self.report(crate::AnimationError {
                location,
                kind: crate::AnimationErrorKind::DuplicateMarker {
                    name: name.clone(),
                    first,
                },
            });
            return None;
        }
        self.markers.insert(name.clone(), location);
        Some(self.cue(Cued::Marker(name.clone())))
    }
    /// A cue for an event. Every use gets its own, so an animation reused
    /// in several places fires its events once per place.
    pub(crate) fn register_event(
        &mut self,
        payload: Arc<dyn Any + Send + Sync>,
        length: Duration,
    ) -> CueId {
        self.cue(Cued::Event(payload, length))
    }
    fn cue(&mut self, cued: Cued) -> CueId {
        let id = u32::try_from(self.cued.len())
            .expect("fewer than 2^32 markers and events");
        self.cued.push(cued);
        CueId(id)
    }
}

/// What a core cue stands for.
enum Cued {
    Marker(crate::MarkerName),
    Event(Arc<dyn Any + Send + Sync>, Duration),
}

/// One event of a compiled scene; see [`CompiledScene::events`].
struct TimedEvent {
    start: Duration,
    length: Duration,
    payload: Arc<dyn Any + Send + Sync>,
}

/// An event read from a [`CompiledScene`], with its payload of type `E`.
#[derive(Debug)]
pub struct Event<'a, E> {
    /// When the event starts.
    pub start: Duration,
    /// How long it lasts on the timeline.
    pub length: Duration,
    /// What it carries, as passed to [`event`](crate::event).
    pub payload: &'a E,
}
// Derived impls would require `E: Clone`/`Copy`; only the reference is
// copied.
impl<E> Clone for Event<'_, E> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E> Copy for Event<'_, E> {}

/// Every marker with its time, sorted by time.
type Markers = Box<[(crate::MarkerName, Duration)]>;

/// Split the compiled track's cues into markers and events, both sorted
/// by time (the track sorts its cues).
fn resolve_cues(
    cues: &[(CueId, Duration)],
    mut cued: Vec<Cued>,
) -> (Markers, Box<[TimedEvent]>) {
    let mut markers = Vec::new();
    let mut events = Vec::new();
    for &(CueId(id), start) in cues {
        // Each cue is used once, so its entry can be moved out.
        let entry = std::mem::replace(
            &mut cued[id as usize],
            Cued::Marker(crate::MarkerName::Borrowed("")),
        );
        match entry {
            Cued::Marker(name) => markers.push((name, start)),
            Cued::Event(payload, length) => events.push(TimedEvent {
                start,
                length,
                payload,
            }),
        }
    }
    (markers.into_boxed_slice(), events.into_boxed_slice())
}
type Prepare<M> = Box<dyn FnOnce(&mut Scene<M>) + Send + Sync>;
type Update<M> =
    dyn FnMut(&mut World<M>, &BindingPlan<World<M>>) + Send + Sync;

/// A callback that runs on every sample, made by [`UpdateTargets`].
pub struct NativeUpdate<M>(Box<Update<M>>);
impl<M> NativeUpdate<M> {
    /// Wrap `update`, which receives the world and the bindings' values.
    pub fn new(
        update: impl FnMut(&mut World<M>, &BindingPlan<World<M>>)
        + ThreadSafe,
    ) -> Self {
        Self(Box::new(update))
    }
}
/// The objects a `scene.update` callback changes: one handle, or a tuple
/// or array of them (see [`UpdateStorage`](crate::UpdateStorage)).
pub trait UpdateTargets<M>: Sized {
    /// What the callback receives, such as `&mut Circle`.
    type Objects<'a>;
    /// Check the targets, then build a callback that reads `source` and
    /// passes its value and the borrowed objects to `update`.
    ///
    /// # Panics
    ///
    /// If a target isn't in this scene or is borrowed twice.
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
/// Something [`Scene::add`] accepts: an object description, or a reusable
/// component that adds several objects and returns its own handle type.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be added to the scene",
    label = "not a scene object",
    note = "pass an object description such as `Circle::new(..)`, or implement `SceneObject` for a component"
)]
pub trait SceneObject<M> {
    /// What `scene.add` returns, used to animate the object.
    type Handle;
    /// Add this object (or component) to `scene`.
    fn insert_into(self, scene: &mut Scene<M>) -> Self::Handle;
}

/// A scene being built: add objects, bind values, then
/// [`compile`](Self::compile) it with its animations.
///
/// `M` is the model that stores the objects; `motiongfx_2d::Scene` is this
/// type with its 2D model filled in, and has an example.
pub struct Scene<M> {
    world: World<M>,
    bindings: BindingPlan<World<M>>,
    updates: Vec<NativeUpdate<M>>,
    preparation: Vec<(TypeId, Prepare<M>)>,
    defaults: crate::AnimationDefaults,
    simulations: Vec<Box<dyn Stepper<M>>>,
    simulation_rate: u32,
}
impl<M: Default + 'static> Default for Scene<M> {
    fn default() -> Self {
        Self::with_model(M::default())
    }
}
impl<M: Default + 'static> Scene<M> {
    /// An empty scene.
    pub fn new() -> Self {
        Self::default()
    }
}
impl<M: 'static> Scene<M> {
    /// A scene starting from `model`.
    pub fn with_model(model: M) -> Self {
        Self {
            world: World {
                model,
                signals: SignalValues::default(),
                simulations: SimulationStates::default(),
                time: Duration::ZERO,
            },
            bindings: BindingPlan::default(),
            updates: Vec::new(),
            preparation: Vec::new(),
            defaults: crate::AnimationDefaults::default(),
            simulations: Vec::new(),
            simulation_rate: DEFAULT_RATE,
        }
    }
    /// Duration and ease for every animation that does not set its own.
    /// Read when the scene compiles, so it applies to all animations,
    /// including ones built before this call.
    pub fn defaults(&mut self, duration: Duration, ease: EaseFn) {
        self.defaults = crate::AnimationDefaults { duration, ease };
    }
    /// How many steps per second every simulation in this scene takes
    /// (60 unless set). Each step's closure receives `dt = 1 / rate`
    /// seconds. Read when the scene compiles, so it applies to simulations
    /// added before this call too.
    ///
    /// Keep it at or above the frame rate you play or export at: a frame
    /// shows the latest step, so a slower rate repeats states.
    ///
    /// # Panics
    ///
    /// If `steps_per_second` is zero.
    #[track_caller]
    pub fn simulation_rate(&mut self, steps_per_second: u32) {
        assert!(
            steps_per_second > 0,
            "a simulation needs at least one step per second"
        );
        self.simulation_rate = steps_per_second;
    }
    /// A value that advances in fixed steps from its previous state, such
    /// as a falling ball's height and speed. `step` receives the state to
    /// change and the step length `dt` in seconds (see
    /// [`simulation_rate`](Self::simulation_rate)).
    ///
    /// The simulation runs once when the scene compiles, from time 0 to the
    /// scene's end, and stores every step; sampling looks the state up, so
    /// seeking is exact in both directions. Keep the state small: it is
    /// stored once per step.
    ///
    /// # Examples
    ///
    /// ```
    /// use motiongfx::prelude::s;
    /// use motiongfx_authoring::{Scene, wait};
    ///
    /// let mut scene = Scene::<()>::new();
    /// // Falls from rest under gravity.
    /// let height = scene.simulate((100.0, 0.0), |(height, speed): &mut (f64, f64), dt| {
    ///     *speed -= 9.8 * dt;
    ///     *height += *speed * dt;
    /// });
    /// let mut animation = scene.compile([wait(s(1))]);
    /// animation.sample(s(1));
    /// let &(fallen, _) = animation.simulated(height);
    /// assert!(fallen < 100.0);
    /// ```
    pub fn simulate<T, F>(
        &mut self,
        initial: T,
        mut step: F,
    ) -> Simulated<T>
    where
        T: Clone + PartialEq + ThreadSafe,
        F: FnMut(&mut T, f64) + ThreadSafe,
    {
        let handle = self.world.simulations.add(initial);
        self.simulations.push(Box::new(Simulation {
            handle,
            read: |_: &World<M>, _: &BindingPlan<World<M>>| (),
            step: move |state: &mut T, (), dt| step(state, dt),
        }));
        handle
    }
    /// Like [`simulate`](Self::simulate), but each step also reads
    /// `inputs` (a signal, computed value, property, or a tuple of them) as
    /// they are at that step's time, so the timeline can change what the
    /// simulation does. `step` receives the state, the inputs' values and
    /// `dt` in seconds.
    ///
    /// # Panics
    ///
    /// If `inputs` reads a property that doesn't exist in this scene.
    #[track_caller]
    pub fn simulate_with<T, S, F>(
        &mut self,
        initial: T,
        inputs: S,
        step: F,
    ) -> Simulated<T>
    where
        T: Clone + PartialEq + ThreadSafe,
        S: Source<World<M>>,
        F: FnMut(&mut T, S::Value, f64) + ThreadSafe,
    {
        let inputs =
            inputs.register(&mut self.world, &mut self.bindings);
        self.bindings
            .validate_source(&self.world, &inputs)
            .unwrap_or_else(|error| {
                panic!("invalid simulation input: {error}")
            });
        let handle = self.world.simulations.add(initial);
        self.simulations.push(Box::new(Simulation {
            handle,
            read: move |world: &World<M>, bindings: &BindingPlan<World<M>>| {
                bindings
                    .read_source(world, &inputs)
                    .expect("validated simulation input exists")
            },
            step,
        }));
        handle
    }
    /// The model: the objects added so far.
    pub fn model(&self) -> &M {
        self.world.model()
    }
    /// The model, mutably.
    pub fn model_mut(&mut self) -> &mut M {
        self.world.model_mut()
    }
    /// Add an object and return its typed handle. A handle is an index
    /// into this scene: use it only with this scene and the
    /// [`CompiledScene`] compiled from it.
    pub fn add<T: SceneObject<M>>(&mut self, object: T) -> T::Handle {
        object.insert_into(self)
    }
    /// The sample time as a source, for values that depend on time
    /// directly.
    pub fn time(&self) -> crate::SampleTime {
        crate::SampleTime
    }
    /// Run `prepare` just before the scene compiles. A later call with the
    /// same key `K` replaces the earlier one, which suits settings where the
    /// last call wins, such as the background colour.
    pub fn prepare_with<K: 'static>(
        &mut self,
        prepare: impl FnOnce(&mut Self) + ThreadSafe,
    ) {
        self.prepare_keyed(TypeId::of::<K>(), prepare);
    }
    pub(crate) fn prepare_keyed(
        &mut self,
        key: TypeId,
        prepare: impl FnOnce(&mut Self) + ThreadSafe,
    ) {
        self.preparation.retain(|(previous, _)| *previous != key);
        self.preparation.push((key, Box::new(prepare)));
    }
    /// Set `destination` to `calculate(value)` of `source` after each
    /// sample, recalculating only when the value changes. Object builders
    /// usually do this for you (`Circle::new(&radius)`); use `bind` to wire
    /// objects that already exist.
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
    /// Run `update` on every sample, after the bindings, with `source`'s
    /// value and mutable access to `targets` (one handle, or a tuple or
    /// array of them). Updates run in the order they were added.
    ///
    /// Set fields from the value rather than accumulating changes from one
    /// frame to the next, so seeking backwards gives the same result.
    ///
    /// # Panics
    ///
    /// At the caller's line, if the source or targets are not in this scene.
    #[track_caller]
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
            .unwrap_or_else(|error| {
                panic!("invalid update source: {error}")
            });
        self.updates.push(targets.callback(
            source,
            update,
            &self.world,
        ));
    }
    /// Compile animations into a playable [`CompiledScene`]. The steps run
    /// in order, like [`chain`](crate::chain).
    ///
    /// # Panics
    ///
    /// On any problem [`try_compile`](Self::try_compile) reports, listing
    /// every one with its file and line.
    #[track_caller]
    pub fn compile(
        self,
        steps: impl IntoIterator<Item = crate::Animation<M>>,
    ) -> CompiledScene<M> {
        self.try_compile(steps).unwrap_or_else(|error| {
            panic!(
                "invalid scene preparation:
{error}"
            )
        })
    }
    /// Compile animations into a playable [`CompiledScene`], or return
    /// every problem found. The steps run in order, like
    /// [`chain`](crate::chain).
    ///
    /// # Errors
    ///
    /// [`CompileError::Animation`] with every animation problem found (each
    /// with its file and line), or [`CompileError::Binding`] for an invalid
    /// binding.
    pub fn try_compile(
        mut self,
        steps: impl IntoIterator<Item = crate::Animation<M>>,
    ) -> Result<CompiledScene<M>, CompileError> {
        for (_, prepare) in std::mem::take(&mut self.preparation) {
            prepare(&mut self);
        }
        let mut registry = Registry::new();
        let mut context = CompileContext {
            world: &mut self.world,
            builder: registry.create_builder(),
            defaults: self.defaults,
            errors: Vec::new(),
            actions: Vec::new(),
            markers: Default::default(),
            cued: Vec::new(),
            caches: Default::default(),
        };
        // Lower each step as it is read; the list is never copied.
        let fragment = steps
            .into_iter()
            .map(|step| step.lower(&mut context))
            .ord_chain();
        let CompileContext {
            builder,
            mut errors,
            actions,
            cued,
            ..
        } = context;
        let track = fragment.compile();
        errors.extend(overlaps(&track, &actions));
        if !errors.is_empty() {
            return Err(CompileError::Animation(errors));
        }
        let (markers, events) = resolve_cues(track.cues(), cued);
        let mut timeline = builder.compile(track);
        self.bindings.prepare(&self.world, &timeline)?;
        timeline.bake_actions(&registry, &self.world);
        let mut animation = CompiledScene {
            world: self.world,
            registry,
            timeline,
            bindings: self.bindings,
            updates: self.updates,
            markers,
            events,
        };
        animation
            .run_simulations(self.simulations, self.simulation_rate);
        animation.sample(Duration::ZERO);
        Ok(animation)
    }
}
impl<M> std::fmt::Debug for Scene<M> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("updates", &self.updates.len())
            .finish_non_exhaustive()
    }
}
/// Core keeps only the later-written of two overlapping changes to
/// aliasing fields. Report each dropped change at its own line, naming the
/// change that would replace it.
fn overlaps(
    track: &Track,
    actions: &[(ActionId, &'static std::panic::Location<'static>)],
) -> Vec<crate::AnimationError> {
    let conflicts = track.conflicts();
    if conflicts.is_empty() {
        return Vec::new();
    }
    let lines: std::collections::HashMap<_, _> =
        actions.iter().copied().collect();
    conflicts
        .iter()
        .map(|conflict| crate::AnimationError {
            location: lines[&conflict.dropped],
            kind: crate::AnimationErrorKind::Overlap {
                field: conflict.dropped_field.field_path(),
                later: lines[&conflict.winner],
            },
        })
        .collect()
}
/// A compiled scene, ready to [`sample`](Self::sample) at any time,
/// forwards or backwards. Renderers draw [`model`](Self::model).
pub struct CompiledScene<M> {
    world: World<M>,
    registry: Registry,
    timeline: Timeline<World<M>>,
    bindings: BindingPlan<World<M>>,
    updates: Vec<NativeUpdate<M>>,
    markers: Markers,
    /// Sorted by start time.
    events: Box<[TimedEvent]>,
}
impl<M: 'static> std::fmt::Debug for CompiledScene<M> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("CompiledScene")
            .field("time", &self.world.time)
            .field("duration", &self.duration())
            .field("markers", &self.markers())
            .field("events", &self.events.len())
            .finish_non_exhaustive()
    }
}
impl<M: 'static> CompiledScene<M> {
    /// The world as last sampled, including signal values and time.
    pub fn world(&self) -> &World<M> {
        &self.world
    }
    /// The object `handle` refers to, as last sampled: a `&Circle` for a
    /// circle's handle, and so on.
    ///
    /// # Panics
    ///
    /// If the handle is from another scene that has more objects of its
    /// type. A handle from a scene with fewer objects reads the wrong one.
    #[track_caller]
    pub fn get<H: ObjectHandle<M>>(&self, handle: H) -> &H::State {
        H::State::get(&self.world, handle.id())
            .expect("the handle is from another scene")
    }
    /// The objects as last sampled.
    pub fn model(&self) -> &M {
        self.world.model()
    }
    /// How long the animation runs.
    pub fn duration(&self) -> Duration {
        self.timeline.tracks()[0].duration()
    }
    /// The time of the marker called `name`, if the animation has one.
    pub fn marker(&self, name: &str) -> Option<Duration> {
        self.markers
            .iter()
            .find(|(marker, _)| marker == name)
            .map(|(_, time)| *time)
    }
    /// Every marker with its time, sorted by time.
    pub fn markers(&self) -> &[(crate::MarkerName, Duration)] {
        &self.markers
    }
    /// Every [`event`](crate::event) whose payload is an `E`, sorted by
    /// start time; events of other payload types are skipped.
    ///
    /// # Examples
    ///
    /// ```
    /// use motiongfx::prelude::ms;
    /// use motiongfx_authoring::{Scene, event};
    /// struct Caption(&'static str);
    /// let scene = Scene::<()>::new().compile([
    ///     event(Caption("Hello"), ms(800)),
    ///     event(Caption("World"), ms(800)),
    /// ]);
    /// let starts: Vec<_> = scene
    ///     .events::<Caption>()
    ///     .map(|event| (event.payload.0, event.start))
    ///     .collect();
    /// assert_eq!(starts, [("Hello", ms(0)), ("World", ms(800))]);
    /// ```
    pub fn events<E: 'static>(
        &self,
    ) -> impl Iterator<Item = Event<'_, E>> + '_ {
        self.events.iter().filter_map(|event| {
            Some(Event {
                start: event.start,
                length: event.length,
                payload: event.payload.downcast_ref()?,
            })
        })
    }
    /// The state of `simulation` at the time last sampled.
    ///
    /// # Panics
    ///
    /// If this scene has no simulation of type `T` at the handle's
    /// position, as with a handle from another scene.
    #[track_caller]
    pub fn simulated<T: 'static>(
        &self,
        simulation: Simulated<T>,
    ) -> &T {
        self.world
            .simulations
            .state(simulation, self.world.time)
            .expect("the simulation belongs to this scene")
    }
    /// Run every simulation from time 0 to the end, `rate` steps per
    /// second, storing each step. Each step reads its inputs from the
    /// scene sampled at that step's time.
    fn run_simulations(
        &mut self,
        mut simulations: Vec<Box<dyn Stepper<M>>>,
        rate: u32,
    ) {
        self.world.simulations.set_rate(rate);
        if simulations.is_empty() {
            return;
        }
        let steps = step_index(self.duration(), rate);
        let dt = 1.0 / f64::from(rate);
        for simulation in &simulations {
            simulation.reserve(&mut self.world, steps);
        }
        for index in 0..steps {
            self.sample(step_time(index, rate));
            for simulation in &mut simulations {
                simulation.step(&mut self.world, &self.bindings, dt);
            }
        }
    }
    /// Show the scene at `time` (clamped to the duration): apply the
    /// animations, then the bindings, then the `scene.update` callbacks.
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
