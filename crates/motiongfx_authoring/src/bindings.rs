//! Bindings: object fields calculated from other values after each sample,
//! such as a label that follows a circle.
//!
//! A [`BindingPlan`] holds every binding in a scene. It is checked once
//! ([`BindingPlan::prepare`]: missing properties, two bindings writing one
//! field, bound fields that are also animated, cycles) and then evaluated
//! after each sample, in dependency order. Only bindings whose inputs may
//! have changed run: those reading a sequence the timeline just wrote, the
//! sample time, or a binding that recalculated. A binding whose inputs are
//! unchanged still skips recalculating.

use crate::{Property, source::SourceId};
use motiongfx::action::{ActionKey, ActionTable, UntypedSubjectId};
use motiongfx::prelude::{SubjectSource, ThreadSafe, Timeline};
use motiongfx::subject::SubjectId;
use std::any::{Any, TypeId};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// A binding problem found when the scene compiles.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BindingError {
    /// A binding reads or writes a property that doesn't exist, usually a
    /// handle from another scene.
    MissingProperty(String),
    /// A computed value registered in a different scene.
    ForeignDerived,
    /// Two bindings write the same field (or a point and one of its axes).
    ConflictingWriters(String),
    /// A bound field is also animated; the binding would overwrite it.
    AnimatedDestination(String),
    /// Bindings depend on each other in a loop.
    Cycle,
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProperty(p) => {
                write!(f, "binding property does not exist: {p}")
            }
            Self::ForeignDerived => write!(
                f,
                "derived value belongs to a different binding plan"
            ),
            Self::ConflictingWriters(p) => write!(
                f,
                "multiple bindings write overlapping property {p}"
            ),
            Self::AnimatedDestination(p) => {
                write!(f, "bound property is also animated: {p}")
            }
            Self::Cycle => {
                write!(f, "binding dependencies contain a cycle")
            }
        }
    }
}
impl std::error::Error for BindingError {}

trait SubjectKey: ThreadSafe {
    fn as_any(&self) -> &dyn Any;
    fn equals(&self, other: &dyn SubjectKey) -> bool;
    /// The ID its sequences are keyed by, if the timeline animates it.
    fn untyped(
        &self,
        table: &ActionTable,
    ) -> Option<UntypedSubjectId>;
    fn describe(&self) -> String;
}
impl<I: SubjectId> SubjectKey for I {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn equals(&self, other: &dyn SubjectKey) -> bool {
        other.as_any().downcast_ref::<I>() == Some(self)
    }
    fn untyped(
        &self,
        table: &ActionTable,
    ) -> Option<UntypedSubjectId> {
        table.get_uid(self).map(UntypedSubjectId::new::<I>)
    }
    fn describe(&self) -> String {
        format!("{self:?}")
    }
}

#[derive(Clone)]
struct PropertyKey {
    subject: Arc<dyn SubjectKey>,
    source: TypeId,
    path: &'static str,
}
fn paths_overlap(a: &str, b: &str) -> bool {
    let (short, long) =
        if a.len() <= b.len() { (a, b) } else { (b, a) };
    long.strip_prefix(short)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}
impl PropertyKey {
    fn of<I: SubjectId, S: 'static, T>(
        property: Property<I, S, T>,
    ) -> Self {
        Self {
            subject: Arc::new(property.id),
            source: TypeId::of::<S>(),
            path: property.path.field.field_path(),
        }
    }
    fn overlaps(&self, other: &Self) -> bool {
        self.source == other.source
            && self.subject.equals(&*other.subject)
            && paths_overlap(self.path, other.path)
    }
    fn untyped(
        &self,
        table: &ActionTable,
    ) -> Option<UntypedSubjectId> {
        self.subject.untyped(table)
    }
    fn describe(&self) -> String {
        format!("{}{}", self.subject.describe(), self.path)
    }
}

/// What a binding source reads, so the plan can order bindings.
#[doc(hidden)]
#[derive(Clone)]
pub struct BindingDependency(Dependency);
#[derive(Clone)]
enum Dependency {
    Property(PropertyKey),
    Computed {
        plan: u64,
        index: usize,
    },
    /// The sample time, which changes on every sample that moves it.
    Time,
}
impl BindingDependency {
    /// The sample time, for sources that depend only on when the scene is
    /// sampled.
    pub fn time() -> Self {
        Self(Dependency::Time)
    }
    /// The whole object `id` of type `S`: everything stored in it.
    pub fn object<I: SubjectId, S: 'static>(id: I) -> Self {
        Self(Dependency::Property(PropertyKey {
            subject: Arc::new(id),
            source: TypeId::of::<S>(),
            // The object itself; every field path starts with `.`.
            path: "",
        }))
    }
}

/// Cached computed values, read through [`ComputedSlot`]s.
#[doc(hidden)]
#[derive(Default)]
pub struct DerivedValues(Vec<Option<Box<dyn Any + Send + Sync>>>);

/// A registered source a binding reads while sampling: a property, a
/// computed value's [`ComputedSlot`], or a tuple of up to five of these.
/// Implementations must report everything they read in `dependencies`.
pub trait BindingSource<W>: Clone + ThreadSafe {
    /// The value read.
    type Value: Clone + PartialEq + ThreadSafe;
    #[doc(hidden)]
    fn read(
        &self,
        world: &W,
        values: &DerivedValues,
    ) -> Option<Self::Value>;
    #[doc(hidden)]
    fn dependencies(&self) -> Vec<BindingDependency>;
    #[doc(hidden)]
    fn validate(&self, world: &W) -> Result<(), BindingError>;
}

impl<W, I, S, T> BindingSource<W> for Property<I, S, T>
where
    I: SubjectId,
    S: 'static,
    T: Clone + PartialEq + ThreadSafe,
    W: SubjectSource<I, S>,
{
    type Value = T;
    fn read(&self, world: &W, _: &DerivedValues) -> Option<T> {
        world
            .get_source(self.id)
            .map(|s| self.path.lens.get_ref(s).clone())
    }
    fn dependencies(&self) -> Vec<BindingDependency> {
        vec![BindingDependency(Dependency::Property(
            PropertyKey::of(*self),
        ))]
    }
    fn validate(&self, world: &W) -> Result<(), BindingError> {
        world.get_source(self.id).map(|_| ()).ok_or_else(|| {
            BindingError::MissingProperty(
                PropertyKey::of(*self).describe(),
            )
        })
    }
}

/// A computed value registered in one [`BindingPlan`]; reads its cached
/// result.
pub struct ComputedSlot<T> {
    plan: u64,
    index: usize,
    marker: PhantomData<fn() -> T>,
}
impl<T> Copy for ComputedSlot<T> {}
impl<T> Clone for ComputedSlot<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<W, T: Clone + PartialEq + ThreadSafe> BindingSource<W>
    for ComputedSlot<T>
{
    type Value = T;
    fn read(&self, _: &W, values: &DerivedValues) -> Option<T> {
        values
            .0
            .get(self.index)?
            .as_ref()?
            .downcast_ref::<T>()
            .cloned()
    }
    fn dependencies(&self) -> Vec<BindingDependency> {
        vec![BindingDependency(Dependency::Computed {
            plan: self.plan,
            index: self.index,
        })]
    }
    fn validate(&self, _: &W) -> Result<(), BindingError> {
        Ok(())
    }
}

macro_rules! tuple_source {
    ($($name:ident:$index:tt),+) => {
        impl<W, $($name: BindingSource<W>),+> BindingSource<W> for ($($name,)+) {
            type Value = ($($name::Value,)+);
            fn read(&self, world: &W, values: &DerivedValues) -> Option<Self::Value> {
                Some(($(self.$index.read(world, values)?,)+))
            }
            fn dependencies(&self) -> Vec<BindingDependency> {
                let mut dependencies = Vec::new();
                $(dependencies.extend(self.$index.dependencies());)+
                dependencies
            }
            fn validate(&self, world: &W) -> Result<(), BindingError> {
                $(self.$index.validate(world)?;)+
                Ok(())
            }
        }
    }
}
tuple_source!(A:0);
tuple_source!(A:0, B:1);
tuple_source!(A:0, B:1, C:2);
tuple_source!(A:0, B:1, C:2, D:3);
tuple_source!(A:0, B:1, C:2, D:3, E:4);

type Evaluate<W> = Box<
    dyn FnMut(&mut W, &mut DerivedValues, bool) -> bool + Send + Sync,
>;
type Validate<W> =
    Box<dyn Fn(&W) -> Result<(), BindingError> + Send + Sync>;

/// Why a node runs in an evaluation, as bit flags.
type Mark = u8;
/// One of its inputs may have changed: it compares its input first and
/// skips if it is the same.
const SOURCE: Mark = 1;
/// Something else wrote one of a `bind_mut`'s targets: it writes again
/// even if its input is the same, to keep the last word.
const TARGET: Mark = 2;

/// Sorts `(node, mark)` pairs by node and merges each node's marks.
fn merge_marks(pairs: &mut Vec<(usize, Mark)>) {
    pairs.sort_unstable_by_key(|&(node, _)| node);
    pairs.dedup_by(|later, kept| {
        let same = later.0 == kept.0;
        if same {
            kept.1 |= later.1;
        }
        same
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bind,
    Computed,
    BindMut,
}
struct Node<W> {
    kind: Kind,
    dependencies: Vec<BindingDependency>,
    /// What it writes: a bound field, or a `bind_mut`'s whole objects.
    destinations: Vec<PropertyKey>,
    validate: Validate<W>,
    /// Runs the node, or with `force` runs it even if its input is
    /// unchanged. Returns whether its output may have changed, so the
    /// nodes reading it should run.
    evaluate: Evaluate<W>,
}

/// Every binding and computed value in one scene. Add them with
/// [`bind`](Self::bind), [`bind_mut`](Self::bind_mut) and
/// [`computed`](Self::computed), check them once with
/// [`prepare`](Self::prepare), then call [`evaluate`](Self::evaluate)
/// after each sample. A [`Scene`](crate::Scene) does all of this for you.
pub struct BindingPlan<W> {
    id: u64,
    registered: HashMap<SourceId, usize>,
    nodes: Vec<Node<W>>,
    /// Evaluation order: every node after the nodes it reads.
    order: Vec<usize>,
    /// The nodes that read each node's output, and how to mark them.
    dependents: Vec<Vec<(usize, Mark)>>,
    /// The nodes to run when the timeline writes a sequence, and how to
    /// mark them.
    watchers: HashMap<ActionKey, Box<[(usize, Mark)]>>,
    /// Subject types with watched sequences, so sequences of other types
    /// are skipped without a lookup.
    watched_types: Vec<TypeId>,
    /// The nodes that read the sample time.
    timed: Vec<usize>,
    /// Why each node runs in the current evaluation; 0 if it doesn't.
    dirty: Vec<Mark>,
    values: DerivedValues,
    prepared: bool,
    first_evaluation: bool,
}
impl<W> Default for BindingPlan<W> {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            registered: HashMap::new(),
            nodes: Vec::new(),
            order: Vec::new(),
            dependents: Vec::new(),
            watchers: HashMap::new(),
            watched_types: Vec::new(),
            timed: Vec::new(),
            dirty: Vec::new(),
            values: DerivedValues::default(),
            prepared: false,
            first_evaluation: true,
        }
    }
}

impl<W: 'static> BindingPlan<W> {
    pub(crate) fn registered_computed<T>(
        &self,
        id: SourceId,
    ) -> Option<ComputedSlot<T>> {
        self.registered.get(&id).map(|&index| ComputedSlot {
            plan: self.id,
            index,
            marker: PhantomData,
        })
    }
    pub(crate) fn remember_computed<T>(
        &mut self,
        id: SourceId,
        slot: ComputedSlot<T>,
    ) {
        self.registered.insert(id, slot.index);
    }
    /// The current value of `source`, after [`evaluate`](Self::evaluate).
    pub fn read_source<S: BindingSource<W>>(
        &self,
        world: &W,
        source: &S,
    ) -> Option<S::Value> {
        source.read(world, &self.values)
    }

    /// Check a source read outside the plan (such as by a simulation):
    /// its properties exist and its computed values belong to this plan.
    ///
    /// # Errors
    ///
    /// The first problem found.
    pub fn validate_source<S: BindingSource<W>>(
        &self,
        world: &W,
        source: &S,
    ) -> Result<(), BindingError> {
        source.validate(world)?;
        for BindingDependency(dependency) in source.dependencies() {
            if let Dependency::Computed { plan, index } = dependency
                && !self.is_own_computed(plan, index)
            {
                return Err(BindingError::ForeignDerived);
            }
        }
        Ok(())
    }

    fn is_own_computed(&self, plan: u64, index: usize) -> bool {
        plan == self.id
            && self
                .nodes
                .get(index)
                .is_some_and(|node| node.kind == Kind::Computed)
    }

    /// Set `destination` to `calculate(value)` whenever `source`'s value
    /// changes. `calculate` should depend only on its input, so seeking
    /// reproduces the same result.
    pub fn bind<I, S, T, Source, F>(
        &mut self,
        destination: impl Into<Property<I, S, T>>,
        source: Source,
        calculate: F,
    ) where
        I: SubjectId,
        S: 'static,
        T: Clone + ThreadSafe,
        W: SubjectSource<I, S>,
        Source: BindingSource<W>,
        F: Fn(Source::Value) -> T + ThreadSafe,
    {
        self.prepared = false;
        let destination = destination.into();
        let validation_source = source.clone();
        let mut previous = None;
        self.nodes.push(Node {
            kind: Kind::Bind,
            dependencies: source.dependencies(),
            destinations: vec![PropertyKey::of(destination)],
            validate: Box::new(move |world| {
                validation_source.validate(world)?;
                world
                    .get_source(destination.id)
                    .map(|_| ())
                    .ok_or_else(|| {
                        BindingError::MissingProperty(
                            PropertyKey::of(destination).describe(),
                        )
                    })
            }),
            evaluate: Box::new(move |world, values, force| {
                let input = source
                    .read(world, values)
                    .expect("validated binding source exists");
                if !force && previous.as_ref() == Some(&input) {
                    return false;
                }
                let output = calculate(input.clone());
                world.apply_source(destination.id, |object| {
                    *destination.path.lens.get_mut(object) = output
                });
                previous = Some(input);
                true
            }),
        });
        self.values.0.push(None);
    }

    /// Run `write` with `source`'s value whenever it changes, or when
    /// anything else writes one of `targets` (whole objects, from
    /// [`BindingDependency::object`]), so `write` always has the last
    /// word on them. `write` should set fields from the value alone, so
    /// seeking reproduces the same result.
    pub fn bind_mut<Source, F>(
        &mut self,
        targets: Vec<BindingDependency>,
        source: Source,
        mut write: F,
    ) where
        Source: BindingSource<W>,
        F: FnMut(&mut W, Source::Value) + ThreadSafe,
    {
        self.prepared = false;
        let destinations = targets
            .into_iter()
            .filter_map(|BindingDependency(target)| match target {
                Dependency::Property(key) => Some(key),
                _ => None,
            })
            .collect();
        let validation_source = source.clone();
        let mut previous = None;
        self.nodes.push(Node {
            kind: Kind::BindMut,
            dependencies: source.dependencies(),
            destinations,
            validate: Box::new(move |world| {
                validation_source.validate(world)
            }),
            // Forced when something else wrote one of its targets.
            evaluate: Box::new(move |world, values, force| {
                let input = source
                    .read(world, values)
                    .expect("validated bind_mut source exists");
                if !force && previous.as_ref() == Some(&input) {
                    return false;
                }
                write(world, input.clone());
                previous = Some(input);
                true
            }),
        });
        self.values.0.push(None);
    }

    /// Cache `calculate(value)` of `source`, recalculated when its value
    /// changes, for other bindings to read through the returned slot.
    pub fn computed<Source, T, F>(
        &mut self,
        source: Source,
        calculate: F,
    ) -> ComputedSlot<T>
    where
        Source: BindingSource<W>,
        T: Clone + PartialEq + ThreadSafe,
        F: Fn(Source::Value) -> T + ThreadSafe,
    {
        self.prepared = false;
        let index = self.nodes.len();
        let validation_source = source.clone();
        let mut previous = None;
        self.nodes.push(Node {
            kind: Kind::Computed,
            dependencies: source.dependencies(),
            destinations: Vec::new(),
            validate: Box::new(move |world| {
                validation_source.validate(world)
            }),
            evaluate: Box::new(move |world, values, force| {
                let input = source
                    .read(world, values)
                    .expect("validated derived source exists");
                if !force && previous.as_ref() == Some(&input) {
                    return false;
                }
                let output = calculate(input.clone());
                previous = Some(input);
                let slot = &mut values.0[index];
                if let Some(cached) = slot.as_mut() {
                    let cached = cached
                        .downcast_mut::<T>()
                        .expect("derived output type is fixed");
                    // An unchanged result, such as a rounded or clamped
                    // value, stops here: nothing reading it needs to run.
                    if !force && *cached == output {
                        return false;
                    }
                    *cached = output;
                } else {
                    *slot = Some(Box::new(output));
                }
                true
            }),
        });
        self.values.0.push(None);
        ComputedSlot {
            plan: self.id,
            index,
            marker: PhantomData,
        }
    }

    /// Check every binding against `world` and `timeline`, work out the
    /// order to evaluate them in, and which timeline sequences each one
    /// reads. Call again after adding bindings.
    ///
    /// # Errors
    ///
    /// A [`BindingError`] for a missing property, two bindings writing one
    /// field, a bound field that the timeline animates, a computed value
    /// from another plan, or a cycle.
    pub fn prepare(
        &mut self,
        world: &W,
        timeline: &Timeline<W>,
    ) -> Result<(), BindingError> {
        self.prepared = false;
        let count = self.nodes.len();
        let table = timeline.action_table();
        // Every animated sequence, grouped by subject and source type, so
        // finding the ones a property overlaps is one lookup.
        let mut sequences: HashMap<
            (UntypedSubjectId, TypeId),
            Vec<ActionKey>,
        > = HashMap::new();
        for track in timeline.tracks() {
            for (key, _) in track.sequences_spans() {
                sequences
                    .entry((
                        *key.subject_id(),
                        key.field().source_id(),
                    ))
                    .or_default()
                    .push(*key);
            }
        }
        let overlapping = |property: &PropertyKey| {
            property
                .untyped(table)
                .and_then(|subject| {
                    sequences.get(&(subject, property.source))
                })
                .into_iter()
                .flatten()
                .filter(|key| {
                    paths_overlap(
                        property.path,
                        key.field().field_path(),
                    )
                })
        };

        // `predecessors[node]` must run before `node`. A `bind_mut` writes
        // whole objects, so ordering its readers after it is held back
        // until the rest is known; see below.
        let mut predecessors: Vec<Vec<(usize, Mark)>> =
            vec![Vec::new(); count];
        let mut bind_mut_readers = Vec::new();
        let mut watchers: HashMap<ActionKey, Vec<(usize, Mark)>> =
            HashMap::new();
        let mut timed = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            (node.validate)(world)?;
            for destination in &node.destinations {
                for (other_index, other) in
                    self.nodes.iter().enumerate().take(index)
                {
                    if !other
                        .destinations
                        .iter()
                        .any(|other| destination.overlaps(other))
                    {
                        continue;
                    }
                    // A `bind_mut` writes after any binding of the same
                    // object, and after earlier `bind_mut`s of it. Two
                    // bindings of one field are ambiguous.
                    match (node.kind, other.kind) {
                        (Kind::BindMut, _) => {
                            predecessors[index]
                                .push((other_index, TARGET));
                        }
                        (_, Kind::BindMut) => {
                            predecessors[other_index]
                                .push((index, TARGET));
                        }
                        _ => {
                            return Err(
                                BindingError::ConflictingWriters(
                                    destination.describe(),
                                ),
                            );
                        }
                    }
                }
                let mut animated =
                    overlapping(destination).peekable();
                match node.kind {
                    Kind::Bind if animated.peek().is_some() => {
                        return Err(
                            BindingError::AnimatedDestination(
                                destination.describe(),
                            ),
                        );
                    }
                    // A `bind_mut` writes again after the timeline does.
                    Kind::BindMut => {
                        for key in animated {
                            watchers
                                .entry(*key)
                                .or_default()
                                .push((index, TARGET));
                        }
                    }
                    _ => {}
                }
            }
            for BindingDependency(dependency) in &node.dependencies {
                match dependency {
                    Dependency::Computed {
                        plan,
                        index: computed,
                    } => {
                        if !self.is_own_computed(*plan, *computed) {
                            return Err(BindingError::ForeignDerived);
                        }
                        predecessors[index].push((*computed, SOURCE));
                    }
                    Dependency::Property(source) => {
                        for key in overlapping(source) {
                            watchers
                                .entry(*key)
                                .or_default()
                                .push((index, SOURCE));
                        }
                        for (writer, other) in
                            self.nodes.iter().enumerate()
                        {
                            if !other
                                .destinations
                                .iter()
                                .any(|d| d.overlaps(source))
                            {
                                continue;
                            }
                            if other.kind == Kind::BindMut {
                                // A `bind_mut` reading its own target sees
                                // the value it wrote last time.
                                if writer != index {
                                    bind_mut_readers
                                        .push((writer, index));
                                }
                            } else {
                                // Reading its own destination is a cycle.
                                predecessors[index]
                                    .push((writer, SOURCE));
                            }
                        }
                    }
                    Dependency::Time => timed.push(index),
                }
            }
        }
        // A binding reading a `bind_mut`'s object runs after it, unless the
        // `bind_mut` depends on that binding: then it reads the object as
        // it was before the `bind_mut` wrote. Usually the two touch
        // different fields, such as one binding `x` and the other `y`.
        let mut feeds = Vec::new();
        for (writer, reader) in bind_mut_readers {
            feeds.clear();
            feeds.push(writer);
            let mut seen = vec![false; count];
            while let Some(node) = feeds.pop() {
                for &(predecessor, _) in &predecessors[node] {
                    if !std::mem::replace(
                        &mut seen[predecessor],
                        true,
                    ) {
                        feeds.push(predecessor);
                    }
                }
            }
            if !seen[reader] {
                predecessors[reader].push((writer, SOURCE));
            }
        }

        // Kahn's algorithm, taking the earliest-added ready node first, so
        // independent nodes run in the order they were added.
        let mut dependents = vec![Vec::new(); count];
        let mut incoming = vec![0; count];
        for (index, predecessors) in
            predecessors.iter_mut().enumerate()
        {
            merge_marks(predecessors);
            incoming[index] = predecessors.len();
            for &(predecessor, mark) in predecessors.iter() {
                dependents[predecessor].push((index, mark));
            }
        }
        let mut ready: BinaryHeap<Reverse<usize>> = incoming
            .iter()
            .enumerate()
            .filter_map(|(i, &n)| (n == 0).then_some(Reverse(i)))
            .collect();
        let mut order = Vec::with_capacity(count);
        while let Some(Reverse(index)) = ready.pop() {
            order.push(index);
            for &(dependent, _) in &dependents[index] {
                incoming[dependent] -= 1;
                if incoming[dependent] == 0 {
                    ready.push(Reverse(dependent));
                }
            }
        }
        if order.len() != count {
            return Err(BindingError::Cycle);
        }

        timed.dedup();
        let mut watched_types = Vec::new();
        self.watchers = watchers
            .into_iter()
            .map(|(key, mut nodes)| {
                merge_marks(&mut nodes);
                let subject = key.subject_id().type_id();
                if !watched_types.contains(&subject) {
                    watched_types.push(subject);
                }
                (key, nodes.into_boxed_slice())
            })
            .collect();
        self.watched_types = watched_types;
        self.timed = timed;
        self.order = order;
        self.dependents = dependents;
        self.dirty = vec![0; count];
        self.prepared = true;
        self.first_evaluation = true;
        Ok(())
    }

    /// Run the bindings whose inputs may have changed: those reading a
    /// sequence `timeline` just sampled, those reading the sample time if
    /// `time_changed`, and those reading a binding that recalculated. The
    /// first call runs every binding. Call after each timeline sample.
    ///
    /// # Panics
    ///
    /// If [`prepare`](Self::prepare) hasn't succeeded since the last change.
    pub fn evaluate(
        &mut self,
        world: &mut W,
        timeline: &Timeline<W>,
        time_changed: bool,
    ) {
        assert!(self.prepared, "prepare bindings before evaluation");
        let reset = self.first_evaluation;
        self.first_evaluation = false;
        if !reset {
            // Whichever is smaller: checking each watched sequence, or
            // each written one. A scene animating thousands of objects
            // usually watches far fewer sequences than it writes.
            if self.watchers.len() <= timeline.queued_len() {
                for (key, nodes) in &self.watchers {
                    if timeline.is_queued(key) {
                        for &(node, mark) in nodes.iter() {
                            self.dirty[node] |= mark;
                        }
                    }
                }
            } else {
                for key in timeline.queued_keys() {
                    if !self
                        .watched_types
                        .contains(&key.subject_id().type_id())
                    {
                        continue;
                    }
                    for &(node, mark) in
                        self.watchers.get(key).into_iter().flatten()
                    {
                        self.dirty[node] |= mark;
                    }
                }
            }
            if time_changed {
                for &node in &self.timed {
                    self.dirty[node] |= SOURCE;
                }
            }
        }
        for &index in &self.order {
            // Cleared even when resetting, so marks made by this pass don't
            // carry over to the next.
            let mark = std::mem::take(&mut self.dirty[index]);
            if !reset && mark == 0 {
                continue;
            }
            let force = reset || mark & TARGET != 0;
            if (self.nodes[index].evaluate)(
                world,
                &mut self.values,
                force,
            ) {
                for &(dependent, mark) in &self.dependents[index] {
                    self.dirty[dependent] |= mark;
                }
            }
        }
    }
}
