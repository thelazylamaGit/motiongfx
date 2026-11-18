//! [`Animation`] values, the group functions that combine them, and how
//! `compile` turns them into core timeline fragments.
//!
//! An `Animation` is a description: nothing happens until
//! [`Scene::compile`](crate::Scene::compile) lowers it, so animations can be
//! built anywhere, returned from functions, stored, cloned and reused.
//! Lowering walks the tree once, in order, combining fragments with core's
//! own `chain`/`all`/`any`/`flow`. Markers and events lower to core cues,
//! which those combinators shift like clips. See
//! `docs/authoring-redesign.md` for the representation and its costs.

use crate::{
    CompileContext, Field, Interp, Property, Spring, SpringError,
    Target, World,
};
use motiongfx::prelude::*;
use motiongfx::subject::SubjectId;
use std::{
    any::Any, borrow::Cow, panic::Location, sync::Arc, time::Duration,
};

/// The name of a [`marker`]. Literal names are borrowed, not allocated.
pub type MarkerName = Cow<'static, str>;

/// One animation: a property, signal or text-selection change, a group,
/// a wait, a marker or an event. Build them with property methods
/// (`square.shift(..)`, `signal.to(..)`), [`chain`], [`all`], [`any`],
/// [`flow`], [`wait`], [`marker`] and [`event`].
///
/// `M` is the scene's model. `motiongfx_2d` exports `Animation` with its
/// 2D model filled in, so 2D code writes plain `Animation`.
///
/// ```
/// use motiongfx::prelude::{ms, s};
/// use motiongfx_authoring::{Animation, all, chain, signal, wait};
/// let x = signal(0.0);
/// let y = signal(0.0);
/// let move_then_rise: Animation<()> = chain([
///     x.to(100.0).over(ms(500)),
///     wait(ms(200)),
///     all([x.to(0.0), y.to(50.0)]).delay(s(1)),
/// ]);
/// ```
#[must_use = "an animation does nothing until it is passed to `scene.compile(..)` or a group"]
pub struct Animation<M>(Node<M>);

enum Node<M> {
    Step(Arc<dyn Step<M>>, Timing),
    Group(Order, Vec<Animation<M>>),
    Wait(Duration),
    Marker(MarkerName, &'static Location<'static>),
    /// A payload for another crate (such as a sound) and how long it
    /// lasts on the timeline.
    Event(Arc<dyn Any + Send + Sync>, Duration),
    /// Every change inside may interrupt a running animation of the same
    /// property; see [`Animation::interrupt`]. Boxed so the common nodes
    /// stay small.
    Interrupt(Box<Animation<M>>),
}

#[derive(Clone, Copy)]
enum Order {
    Chain,
    All,
    Any,
    Flow(Duration),
}

impl<M> Clone for Animation<M> {
    /// Cheap: steps are shared, so only group lists are copied.
    fn clone(&self) -> Self {
        Self(match &self.0 {
            Node::Step(step, timing) => {
                Node::Step(step.clone(), timing.clone())
            }
            Node::Group(order, children) => {
                Node::Group(*order, children.clone())
            }
            Node::Wait(duration) => Node::Wait(*duration),
            Node::Marker(name, location) => {
                Node::Marker(name.clone(), location)
            }
            Node::Event(payload, length) => {
                Node::Event(payload.clone(), *length)
            }
            Node::Interrupt(inner) => Node::Interrupt(inner.clone()),
        })
    }
}

/// Shows the tree's shape and where each change was written, for
/// example `Chain([Step(src/main.rs:10:5), Wait(1s)])`.
impl<M> std::fmt::Debug for Animation<M> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match &self.0 {
            Node::Step(_, timing) => f
                .debug_tuple("Step")
                .field(&format_args!("{}", timing.location))
                .finish(),
            Node::Group(Order::Chain, children) => {
                f.debug_tuple("Chain").field(children).finish()
            }
            Node::Group(Order::All, children) => {
                f.debug_tuple("All").field(children).finish()
            }
            Node::Group(Order::Any, children) => {
                f.debug_tuple("Any").field(children).finish()
            }
            Node::Group(Order::Flow(interval), children) => f
                .debug_tuple("Flow")
                .field(interval)
                .field(children)
                .finish(),
            Node::Wait(duration) => {
                f.debug_tuple("Wait").field(duration).finish()
            }
            Node::Marker(name, _) => {
                f.debug_tuple("Marker").field(name).finish()
            }
            Node::Event(_, length) => {
                f.debug_tuple("Event").field(length).finish()
            }
            Node::Interrupt(inner) => {
                f.debug_tuple("Interrupt").field(inner).finish()
            }
        }
    }
}

/// Timing of a single change: a tween or a spring, plus where the change
/// was written (for errors). Unset tween values fall back to the scene
/// defaults when the animation is compiled.
#[derive(Clone)]
pub struct Timing {
    motion: Motion,
    location: &'static Location<'static>,
}

#[derive(Clone)]
enum Motion {
    Tween {
        over: Option<Duration>,
        ease: Option<EaseFn>,
    },
    /// Boxed: most changes never spring, so they don't carry its size.
    Spring(Box<Spring>),
}

impl Timing {
    /// Where the animation was created, for error messages.
    pub fn location(&self) -> &'static Location<'static> {
        self.location
    }
}

/// How one change adds itself to the core timeline during `compile`.
/// Implemented for object properties and signals here, and for text
/// selections in `motiongfx_2d`. Most implementations resolve their
/// target, then call [`lower_change`].
pub trait Step<M>: Send + Sync + 'static {
    /// Add this change to the timeline with `timing`, returning its
    /// fragment. Report problems with `context.report(..)` and return an
    /// empty fragment.
    fn lower(
        &self,
        context: &mut CompileContext<'_, M>,
        timing: &Timing,
    ) -> TrackFragment;
}

const GROUP_TIMING: &str = "groups, waits and markers have no timing of \
     their own; set .over()/.ease()/.spring() on each animation inside";
const EVENT_TIMING: &str = "an event has a length but no easing or \
     spring; set its length with .over(..)";
const SPRING_TIMING: &str = "a spring sets its own timing, so it cannot be \
     combined with .over()/.ease() or an instant .set(..)";

impl<M: 'static> Animation<M> {
    /// Wrap a custom [`Step`]. Records the caller's location for errors.
    #[track_caller]
    pub fn from_step(step: impl Step<M>) -> Self {
        Self(Node::Step(
            Arc::new(step),
            Timing {
                motion: Motion::Tween {
                    over: None,
                    ease: None,
                },
                location: Location::caller(),
            },
        ))
    }

    /// The timing of a single change. Groups, waits and markers have none
    /// (yet), and events only a length, so timing modifiers on them panic
    /// at the caller's line.
    #[track_caller]
    fn motion(&mut self) -> &mut Motion {
        match &mut self.0 {
            Node::Step(_, timing) => &mut timing.motion,
            Node::Interrupt(inner) => inner.motion(),
            Node::Event(..) => panic!("{EVENT_TIMING}"),
            Node::Group(..) | Node::Wait(_) | Node::Marker(..) => {
                panic!("{GROUP_TIMING}")
            }
        }
    }

    /// How long this change takes. Without it, the scene default applies.
    /// On an [`event`], how long the event lasts instead of the length it
    /// was created with.
    ///
    /// # Panics
    ///
    /// At the caller's line, on a group, wait or marker, or on a spring.
    #[track_caller]
    pub fn over(mut self, duration: Duration) -> Self {
        if let Node::Event(_, length) = &mut self.0 {
            *length = duration;
            return self;
        }
        match self.motion() {
            Motion::Tween { over, .. } => *over = Some(duration),
            Motion::Spring(_) => panic!("{SPRING_TIMING}"),
        }
        self
    }

    /// Easing curve. Without it, the scene default applies.
    ///
    /// # Panics
    ///
    /// At the caller's line, on a group, wait, marker or event, or on a
    /// spring.
    #[track_caller]
    pub fn ease(mut self, ease: EaseFn) -> Self {
        match self.motion() {
            Motion::Tween { ease: slot, .. } => *slot = Some(ease),
            Motion::Spring(_) => panic!("{SPRING_TIMING}"),
        }
        self
    }

    /// Animate with a spring instead of a tween; the spring decides the
    /// duration. Invalid settings, or a spring on a discrete property, are
    /// reported by `compile`.
    ///
    /// # Panics
    ///
    /// At the caller's line, on a group, wait, marker or event, or after
    /// `.over()`, `.ease()` or `.set(..)`.
    #[track_caller]
    pub fn spring(mut self, spring: impl Into<Spring>) -> Self {
        let motion = self.motion();
        if let Motion::Tween { over: Some(_), .. }
        | Motion::Tween { ease: Some(_), .. } = motion
        {
            panic!("{SPRING_TIMING}");
        }
        *motion = Motion::Spring(Box::new(spring.into()));
        self
    }

    /// Start this animation (or group) `duration` later. Same as
    /// `chain([wait(duration), animation])`, so the delay counts towards
    /// the enclosing group's length.
    pub fn delay(self, duration: Duration) -> Self {
        chain([wait(duration), self])
    }

    /// Let this animation take over from a running animation of the same
    /// property instead of overlapping it, which `compile` reports as an
    /// error. On a group, every change inside may take over. Events have
    /// no property to take over, so an event is never cut short: it plays
    /// out even inside an interrupting group.
    ///
    /// The running animation is cut where this one starts: up to there it
    /// follows its own timing, and this one starts from the value it had
    /// reached. Only an animation that started earlier is cut; two that
    /// start at the same moment still overlap. The rest of an earlier
    /// interrupting animation that would start while this one runs (such
    /// as the fade-out after the fade-in it cut) is dropped, so a repeated
    /// "fade in, then out" can retrigger mid-fade. An animation of `x` does
    /// not cut one of the whole `position`; that is still an overlap.
    ///
    /// The value carries over at a cut, but not the speed: this animation
    /// starts its curve from the beginning. For animations that interrupt
    /// often, use an ease-out curve such as `cubic::ease_out`; an ease-in
    /// curve starts so slowly that repeatedly interrupted motion stalls.
    ///
    /// # Examples
    ///
    /// ```
    /// use motiongfx::prelude::{ms, s};
    /// use motiongfx_authoring::{Animation, all, signal};
    /// let x = signal(0.0);
    /// // Heads for 100, but turns back towards 0 after 200 ms.
    /// let turn: Animation<()> = all([
    ///     x.to(100.0).over(s(1)),
    ///     x.to(0.0).over(ms(500)).delay(ms(200)).interrupt(),
    /// ]);
    /// ```
    pub fn interrupt(self) -> Self {
        match self.0 {
            // Nothing inside to cut with.
            Node::Wait(_)
            | Node::Marker(..)
            | Node::Event(..)
            | Node::Interrupt(_) => self,
            node => Self(Node::Interrupt(Box::new(Self(node)))),
        }
    }

    /// Lower this tree into one core fragment, in source order. Child
    /// fragments stream straight into core's combinators.
    pub(crate) fn lower(
        &self,
        context: &mut CompileContext<'_, M>,
    ) -> TrackFragment {
        match &self.0 {
            Node::Step(step, timing) => step.lower(context, timing),
            Node::Group(Order::Flow(interval), children) => {
                lower_flow(*interval, children, context)
            }
            Node::Group(order, children) => {
                let fragments =
                    children.iter().map(|child| child.lower(context));
                match order {
                    Order::Chain => fragments.ord_chain(),
                    Order::Any => fragments.ord_any(),
                    _ => fragments.ord_all(),
                }
            }
            Node::Wait(duration) => TrackFragment::silent(*duration),
            Node::Marker(name, location) => context
                .register_marker(name, location)
                .map_or_else(TrackFragment::new, TrackFragment::cue),
            Node::Event(payload, length) => {
                let id =
                    context.register_event(payload.clone(), *length);
                let mut fragment = TrackFragment::silent(*length);
                fragment.add_cue(id, Duration::ZERO);
                fragment
            }
            Node::Interrupt(inner) => {
                inner.lower(context).interrupting()
            }
        }
    }
}

/// `flow` gives each child a slot `interval` apart, but a marker takes no
/// slot: it names where the next child starts, or the group's end if no
/// child follows it.
fn lower_flow<M: 'static>(
    interval: Duration,
    children: &[Animation<M>],
    context: &mut CompileContext<'_, M>,
) -> TrackFragment {
    let last_slot = children
        .iter()
        .rposition(|child| !matches!(child.0, Node::Marker(..)));
    let mut slots = 0;
    // Each marker's slot start, or `None` for the group's end.
    let mut markers = Vec::new();
    let mut fragment = children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| match &child.0 {
            Node::Marker(name, location) => {
                if let Some(id) =
                    context.register_marker(name, location)
                {
                    let follows =
                        last_slot.is_some_and(|last| index < last);
                    markers.push((
                        id,
                        follows.then(|| interval * slots),
                    ));
                }
                None
            }
            _ => {
                slots += 1;
                Some(child.lower(context))
            }
        })
        .ord_flow(interval);
    for (id, time) in markers {
        let time = time.unwrap_or(fragment.duration());
        fragment.add_cue(id, time);
    }
    fragment
}

fn group<M: 'static>(
    order: Order,
    animations: impl IntoIterator<Item = Animation<M>>,
) -> Animation<M> {
    Animation(Node::Group(order, animations.into_iter().collect()))
}

/// Run animations one after another.
pub fn chain<M: 'static>(
    animations: impl IntoIterator<Item = Animation<M>>,
) -> Animation<M> {
    group(Order::Chain, animations)
}

/// Start animations together; the group lasts as long as the longest.
pub fn all<M: 'static>(
    animations: impl IntoIterator<Item = Animation<M>>,
) -> Animation<M> {
    group(Order::All, animations)
}

/// Start animations together; the group ends when the shortest one does.
///
/// The longer animations keep running past the group's end, so whatever
/// follows in a [`chain`] starts while they finish. With a [`wait`] inside,
/// what follows starts at most that long after the group starts.
///
/// # Examples
///
/// ```
/// use motiongfx::prelude::{ms, s};
/// use motiongfx_authoring::{Animation, any, chain, signal};
/// let x = signal(0.0);
/// let y = signal(0.0);
/// // `y` starts rising 500 ms in, while `x` is still moving.
/// let overlap: Animation<()> = chain([
///     any([x.to(100.0).over(s(2)), y.to(10.0).over(ms(500))]),
///     y.to(50.0),
/// ]);
/// ```
pub fn any<M: 'static>(
    animations: impl IntoIterator<Item = Animation<M>>,
) -> Animation<M> {
    group(Order::Any, animations)
}

/// Start animations `interval` apart; the group lasts until the last ends.
pub fn flow<M: 'static>(
    interval: Duration,
    animations: impl IntoIterator<Item = Animation<M>>,
) -> Animation<M> {
    group(Order::Flow(interval), animations)
}

/// A pause. Inside [`all`] it does not delay siblings, which all start
/// together; use `.delay(..)` for that.
pub fn wait<M: 'static>(duration: Duration) -> Animation<M> {
    Animation(Node::Wait(duration))
}

/// A named point in time, read after compiling with
/// [`CompiledScene::marker`](crate::CompiledScene::marker). In a [`chain`]
/// it is where the previous step ends; in [`all`] and [`any`], the group
/// start; in
/// [`flow`], where the next child starts (it takes no slot). Names must be
/// unique; `compile` reports duplicates with both locations.
#[track_caller]
pub fn marker<M: 'static>(
    name: impl Into<MarkerName>,
) -> Animation<M> {
    Animation(Node::Marker(name.into(), Location::caller()))
}

/// A timed event carrying `payload` for another crate to act on, lasting
/// `length` on the timeline (change it with `.over(..)`).
///
/// Events are how crates such as `motiongfx_audio` put things that aren't
/// property changes, like sounds, on the timeline. Each is placed like any
/// other animation: in a [`chain`] the next step starts after it, and in
/// [`all`] it can lengthen the group. After compiling,
/// [`CompiledScene::events`](crate::CompiledScene::events) lists every
/// event of a payload type with its start time. Sampling ignores events.
///
/// # Examples
///
/// ```
/// use motiongfx::prelude::ms;
/// use motiongfx_authoring::{Animation, chain, event};
/// #[derive(Debug, PartialEq)]
/// struct Caption(&'static str);
/// let captions: Animation<()> = chain([
///     event(Caption("Hello"), ms(800)),
///     event(Caption("World"), ms(800)).over(ms(500)),
/// ]);
/// ```
pub fn event<M: 'static, E: Send + Sync + 'static>(
    payload: E,
    length: Duration,
) -> Animation<M> {
    Animation(Node::Event(Arc::new(payload), length))
}

/// Identifies a property compactly and rebuilds its full description when
/// compiled. A field [`Target`] is just an object ID, so steps built from
/// handles store 8 bytes instead of a whole [`Property`].
pub(crate) trait PropertySource:
    Copy + Send + Sync + 'static
{
    type Id: SubjectId;
    type Source: Clone + ThreadSafe;
    type Value: Clone + ThreadSafe;
    fn property(
        self,
    ) -> Property<Self::Id, Self::Source, Self::Value>;
}

impl<I, S, T> PropertySource for Property<I, S, T>
where
    I: SubjectId,
    S: Clone + ThreadSafe,
    T: Clone + ThreadSafe,
{
    type Id = I;
    type Source = S;
    type Value = T;
    fn property(self) -> Self {
        self
    }
}

impl<I, S, T, F> PropertySource for Target<I, S, T, F>
where
    I: SubjectId,
    S: Clone + ThreadSafe,
    T: Clone + ThreadSafe,
    F: Field<S, T>,
{
    type Id = I;
    type Source = S;
    type Value = T;
    fn property(self) -> Property<I, S, T> {
        Target::property(self)
    }
}

/// An object-field change: the compact target plus the target function.
struct PropertyStep<P, F> {
    source: P,
    target: F,
}

impl<M, P, F> Step<M> for PropertyStep<P, F>
where
    M: 'static,
    P: PropertySource,
    F: Fn(&P::Value) -> P::Value + Clone + ThreadSafe,
    World<M>: SubjectSource<P::Id, P::Source>,
{
    fn lower(
        &self,
        context: &mut CompileContext<'_, M>,
        timing: &Timing,
    ) -> TrackFragment {
        let property = self.source.property();
        lower_change(context, property, self.target.clone(), timing)
    }
}

/// A change to `source`, whose target is calculated from the value at
/// the change's start. Shared by [`Property`] and [`Target`] methods.
#[track_caller]
pub(crate) fn change<M, P, F>(source: P, target: F) -> Animation<M>
where
    M: 'static,
    P: PropertySource,
    F: Fn(&P::Value) -> P::Value + Clone + ThreadSafe,
    World<M>: SubjectSource<P::Id, P::Source>,
{
    Animation::from_step(PropertyStep { source, target })
}

/// Register one change with the core builder: a spring if set (refused
/// on discrete properties), otherwise the change's own duration and ease,
/// falling back to the scene defaults. Problems are reported with the
/// change's location.
pub fn lower_change<M, I, S, T>(
    context: &mut CompileContext<'_, M>,
    property: Property<I, S, T>,
    target: impl Action<T>,
    timing: &Timing,
) -> TrackFragment
where
    M: 'static,
    I: SubjectId,
    S: Clone + ThreadSafe,
    T: Clone + ThreadSafe,
    World<M>: SubjectSource<I, S>,
{
    if let Motion::Spring(_) = timing.motion
        && let Interp::Discrete = property.interp
    {
        return report(
            context,
            timing,
            AnimationErrorKind::DiscreteSpring,
        );
    }
    let action = context
        .builder
        .act_builder(property.id, property.path, target)
        .with_interp(property.interp.function());
    let id = action.id();
    let fragment = match &timing.motion {
        Motion::Spring(spring) => {
            let result = action.with_spring(**spring).try_play();
            match result {
                Ok(fragment) => fragment,
                Err(error) => {
                    return report(
                        context,
                        timing,
                        AnimationErrorKind::Spring(error),
                    );
                }
            }
        }
        Motion::Tween { over, ease } => {
            let duration = over.unwrap_or(context.defaults.duration);
            // An instant change has no in-between to ease. Storing none
            // saves its entry, and a sample at that exact time reaches the
            // value even under an ease that doesn't end at 1.
            if duration.is_zero() {
                action.play(duration)
            } else {
                action
                    .with_ease(ease.unwrap_or(context.defaults.ease))
                    .play(duration)
            }
        }
    };
    // Remembered so `compile` can point an overlap at this line.
    context.actions.push((id, timing.location));
    fragment
}

/// Record a problem with one change; it adds no time to the timeline.
fn report<M: 'static>(
    context: &mut CompileContext<'_, M>,
    timing: &Timing,
    kind: AnimationErrorKind,
) -> TrackFragment {
    context.report(AnimationError {
        location: timing.location,
        kind,
    });
    TrackFragment::silent(Duration::ZERO)
}

/// A problem found while compiling animations, with the source location
/// of the animation that caused it.
#[derive(Debug, Clone)]
pub struct AnimationError {
    /// Where the animation was written.
    pub location: &'static Location<'static>,
    /// What is wrong with it.
    pub kind: AnimationErrorKind,
}

/// What is wrong with an animation; see [`AnimationError`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AnimationErrorKind {
    /// `.spring()` on a property that changes instantly (text content,
    /// caps/joins).
    DiscreteSpring,
    /// Invalid spring settings.
    Spring(SpringError),
    /// A target that could not be resolved (for example a missing text
    /// selection). Downcast it to inspect the concrete error.
    Target(Arc<dyn std::error::Error + Send + Sync>),
    /// A marker name used twice; `first` is where it was first used.
    DuplicateMarker {
        /// The repeated name.
        name: MarkerName,
        /// Where the name was first used.
        first: &'static Location<'static>,
    },
    /// This change overlaps a later-written change (at `later`) on the
    /// same or an overlapping property, such as `position` and
    /// `position_x`. The later one would replace this one entirely.
    Overlap {
        /// The property this change animates.
        field: &'static str,
        /// Where the change that replaces this one was written.
        later: &'static Location<'static>,
    },
}

impl std::fmt::Display for AnimationError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}: ", self.location)?;
        match &self.kind {
            AnimationErrorKind::DiscreteSpring => f.write_str(
                "this property changes instantly and cannot spring; use .over(..) instead",
            ),
            AnimationErrorKind::Spring(error) => error.fmt(f),
            AnimationErrorKind::Target(error) => error.fmt(f),
            AnimationErrorKind::DuplicateMarker { name, first } => write!(
                f,
                "marker \"{name}\" is already used at {first}; marker names must be unique"
            ),
            AnimationErrorKind::Overlap { field, later } => write!(
                f,
                "this animation of `{field}` overlaps a later one at {later}, which would replace it; run them one after another with chain([..]), animate different properties, or add .interrupt() to the later one if it should take over from where this one is (it must start later)"
            ),
        }
    }
}

/// The message already includes the underlying error, so `source()` is
/// empty; match on [`kind`](Self::kind) to inspect it.
impl std::error::Error for AnimationError {}
