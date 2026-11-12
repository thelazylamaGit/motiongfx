//! Typed native relationships evaluated after timeline sampling.
//!
//! Dependencies are explicit and frozen at preparation. Closure input/output
//! types are checked by Rust; writer conflicts and cycles are checked once.

use crate::{Property, source::SourceId};
use motiongfx::action::{ActionKey, ActionTable};
use motiongfx::prelude::{SubjectSource, ThreadSafe, Timeline};
use motiongfx::subject::SubjectId;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Invalid relationships are reported before playback starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    MissingProperty(String),
    ForeignDerived,
    ConflictingWriters(String),
    AnimatedDestination(String),
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
    fn matches_action(
        &self,
        key: &ActionKey,
        table: &ActionTable,
    ) -> bool;
    fn describe(&self) -> String;
}
impl<I: SubjectId> SubjectKey for I {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn equals(&self, other: &dyn SubjectKey) -> bool {
        other.as_any().downcast_ref::<I>() == Some(self)
    }
    fn matches_action(
        &self,
        key: &ActionKey,
        table: &ActionTable,
    ) -> bool {
        key.subject_id().type_id() == TypeId::of::<I>()
            && table.get_id::<I>(&key.subject_id().uid())
                == Some(self)
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
    fn matches_action(
        &self,
        key: &ActionKey,
        table: &ActionTable,
    ) -> bool {
        self.source == key.field().source_id()
            && paths_overlap(self.path, key.field().field_path())
            && self.subject.matches_action(key, table)
    }
    fn describe(&self) -> String {
        format!("{}{}", self.subject.describe(), self.path)
    }
}

/// Opaque dependency metadata used by typed binding sources.
#[doc(hidden)]
#[derive(Clone)]
pub struct BindingDependency(Dependency);
#[derive(Clone)]
enum Dependency {
    Property(PropertyKey),
    Computed { plan: u64, index: usize },
}

/// Type-erased cached outputs. Only typed `ComputedSlot<T>` handles can read them.
#[doc(hidden)]
#[derive(Default)]
pub struct DerivedValues(Vec<Option<Box<dyn Any + Send + Sync>>>);

/// A source can be a property, derived value, or tuple of sources.
/// Implementations should report every value read in `dependencies`.
pub trait BindingSource<W>: Clone + ThreadSafe {
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

/// Read-only handle to one shared, cached native calculation.
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

type Evaluate<W> =
    Box<dyn FnMut(&mut W, &mut DerivedValues, bool) + Send + Sync>;
type Validate<W> =
    Box<dyn Fn(&W) -> Result<(), BindingError> + Send + Sync>;
struct Node<W> {
    dependencies: Vec<BindingDependency>,
    destination: Option<PropertyKey>,
    validate: Validate<W>,
    evaluate: Evaluate<W>,
}

/// Build once, prepare once, evaluate after each timeline sample.
/// Scene ownership for ordinary property handles is not enforced here.
pub struct BindingPlan<W> {
    id: u64,
    registered: HashMap<SourceId, usize>,
    nodes: Vec<Node<W>>,
    order: Vec<usize>,
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
    /// Read a typed source after evaluation, including a cached derived value.
    pub fn read_source<S: BindingSource<W>>(
        &self,
        world: &W,
        source: &S,
    ) -> Option<S::Value> {
        source.read(world, &self.values)
    }

    /// Validate a source used outside the dependency graph, such as an update.
    pub fn validate_source<S: BindingSource<W>>(
        &self,
        world: &W,
        source: &S,
    ) -> Result<(), BindingError> {
        source.validate(world)?;
        for BindingDependency(dependency) in source.dependencies() {
            if let Dependency::Computed { plan, index } = dependency
                && (plan != self.id
                    || index >= self.nodes.len()
                    || self.nodes[index].destination.is_some())
            {
                return Err(BindingError::ForeignDerived);
            }
        }
        Ok(())
    }

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
            dependencies: source.dependencies(),
            destination: Some(PropertyKey::of(destination)),
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
            evaluate: Box::new(move |world, values, reset| {
                let input = source
                    .read(world, values)
                    .expect("validated binding source exists");
                if reset || previous.as_ref() != Some(&input) {
                    let output = calculate(input.clone());
                    world.apply_source(destination.id, |object| {
                        *destination.path.lens.get_mut(object) =
                            output
                    });
                    previous = Some(input);
                }
            }),
        });
        self.values.0.push(None);
    }

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
            dependencies: source.dependencies(),
            destination: None,
            validate: Box::new(move |world| {
                validation_source.validate(world)
            }),
            evaluate: Box::new(move |world, values, reset| {
                let input = source
                    .read(world, values)
                    .expect("validated derived source exists");
                if reset || previous.as_ref() != Some(&input) {
                    let output = calculate(input.clone());
                    if let Some(cached) = values.0[index].as_mut() {
                        *cached
                            .downcast_mut::<T>()
                            .expect("derived output type is fixed") =
                            output;
                    } else {
                        values.0[index] = Some(Box::new(output));
                    }
                    previous = Some(input);
                }
            }),
        });
        self.values.0.push(None);
        ComputedSlot {
            plan: self.id,
            index,
            marker: PhantomData,
        }
    }

    /// Check property existence, conflicting writers, animated destinations
    /// (including raw appended tracks), foreign derived handles, and cycles.
    /// Calculates one dependency order; no graph sorting happens in playback.
    pub fn prepare(
        &mut self,
        world: &W,
        timeline: &Timeline<W>,
    ) -> Result<(), BindingError> {
        self.prepared = false;
        let count = self.nodes.len();
        let mut edges = vec![Vec::new(); count];
        let mut incoming = vec![0; count];
        for (index, node) in self.nodes.iter().enumerate() {
            (node.validate)(world)?;
            if let Some(destination) = &node.destination {
                for other in self
                    .nodes
                    .iter()
                    .take(index)
                    .filter_map(|n| n.destination.as_ref())
                {
                    if destination.overlaps(other) {
                        return Err(
                            BindingError::ConflictingWriters(
                                destination.describe(),
                            ),
                        );
                    }
                }
                for track in timeline.tracks() {
                    if track.sequences_spans().iter().any(
                        |(key, _)| {
                            destination.matches_action(
                                key,
                                timeline.action_table(),
                            )
                        },
                    ) {
                        return Err(
                            BindingError::AnimatedDestination(
                                destination.describe(),
                            ),
                        );
                    }
                }
            }
            let mut predecessors = Vec::new();
            for BindingDependency(dependency) in &node.dependencies {
                match dependency {
                    Dependency::Computed { plan, index } => {
                        if *plan != self.id
                            || *index >= count
                            || self.nodes[*index]
                                .destination
                                .is_some()
                        {
                            return Err(BindingError::ForeignDerived);
                        }
                        predecessors.push(*index);
                    }
                    Dependency::Property(source) => {
                        for (writer, node) in
                            self.nodes.iter().enumerate()
                        {
                            if node
                                .destination
                                .as_ref()
                                .is_some_and(|d| d.overlaps(source))
                            {
                                predecessors.push(writer);
                            }
                        }
                    }
                }
            }
            predecessors.sort_unstable();
            predecessors.dedup();
            incoming[index] = predecessors.len();
            for predecessor in predecessors {
                edges[predecessor].push(index);
            }
        }
        let mut ready: Vec<_> = incoming
            .iter()
            .enumerate()
            .filter_map(|(i, &n)| (n == 0).then_some(i))
            .collect();
        let mut order = Vec::with_capacity(count);
        while let Some(index) = ready.pop() {
            order.push(index);
            for &dependent in &edges[index] {
                incoming[dependent] -= 1;
                if incoming[dependent] == 0 {
                    ready.push(dependent);
                }
            }
        }
        if order.len() != count {
            return Err(BindingError::Cycle);
        }
        self.order = order;
        self.prepared = true;
        self.first_evaluation = true;
        Ok(())
    }

    /// Call after sampling. Unchanged inputs skip closure execution and writes.
    /// Functions must be deterministic functions of their declared inputs.
    pub fn evaluate(&mut self, world: &mut W) {
        assert!(self.prepared, "prepare bindings before evaluation");
        for &index in &self.order {
            (self.nodes[index].evaluate)(
                world,
                &mut self.values,
                self.first_evaluation,
            );
        }
        self.first_evaluation = false;
    }
}
