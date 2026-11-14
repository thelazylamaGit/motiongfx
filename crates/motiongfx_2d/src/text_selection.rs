//! Deferred, source-aware paint targets. Registration is construction work only.
use crate::*;
use motiongfx::prelude::{SubjectSource, ThreadSafe, TrackFragment};
use motiongfx_authoring::{
    AnimationError, AnimationErrorKind, Field, Property,
    SceneContext, Step, Timing, lower_change,
};
use std::{
    collections::HashMap, marker::PhantomData, ops::Range,
    panic::Location, sync::Arc, time::Duration,
};

#[derive(Clone)]
pub struct TextSelection {
    pub range: TextRange,
    pub fill: Color,
    pub opacity: f64,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct TextSelectionId(usize);
impl TextSelectionId {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Selector {
    Literal { needle: String, occurrence: usize },
    Range(Range<usize>),
}
#[derive(Debug, PartialEq, Eq, Hash)]
struct Query {
    text: TextId,
    selector: Selector,
}
#[derive(Default)]
struct Selections(HashMap<Arc<Query>, TextSelectionId>);

/// Reusable deferred selection. No source lookup or scene mutation occurs here.
/// Only scheduled selections are validated/registered, before timeline baking.
#[derive(Clone)]
pub struct TextSelectionHandle {
    query: Arc<Query>,
    location: &'static Location<'static>,
}
/// A deferred selection field, resolved to an ordinary Property during authoring.
pub struct SelectionProperty<T, F> {
    query: Arc<Query>,
    location: &'static Location<'static>,
    marker: PhantomData<fn() -> (T, F)>,
}
impl<T, F> Clone for SelectionProperty<T, F> {
    fn clone(&self) -> Self {
        Self {
            query: self.query.clone(),
            location: self.location,
            marker: PhantomData,
        }
    }
}
impl TextSelectionHandle {
    fn property<T, F: Field<TextSelection, T>>(
        &self,
    ) -> SelectionProperty<T, F> {
        SelectionProperty {
            query: self.query.clone(),
            location: self.location,
            marker: PhantomData,
        }
    }
    pub fn fill(
        &self,
    ) -> SelectionProperty<Color, fields::fill::Marker> {
        self.property()
    }
    pub fn opacity(
        &self,
    ) -> SelectionProperty<f64, fields::opacity::Marker> {
        self.property()
    }
    #[track_caller]
    pub fn fill_to<M: Model2d>(
        &self,
        color: impl Into<Color>,
    ) -> motiongfx_authoring::Animation<M> {
        self.fill().to(color)
    }
    #[track_caller]
    pub fn set_fill<M: Model2d>(
        &self,
        color: impl Into<Color>,
    ) -> motiongfx_authoring::Animation<M> {
        self.fill().set(color)
    }
}
impl TextHandle {
    /// Describe the first non-overlapping literal occurrence in initial content.
    #[track_caller]
    pub fn select(self, needle: &str) -> TextSelectionHandle {
        self.select_nth(needle, 0)
    }
    /// Occurrences are zero-based. Repeated equivalent selections share paint state.
    #[track_caller]
    pub fn select_nth(
        self,
        needle: &str,
        occurrence: usize,
    ) -> TextSelectionHandle {
        self.selection(Selector::Literal {
            needle: needle.into(),
            occurrence,
        })
    }
    /// Describe a UTF-8 byte range; preparation validates its boundaries.
    #[track_caller]
    pub fn select_range(
        self,
        range: Range<usize>,
    ) -> TextSelectionHandle {
        self.selection(Selector::Range(range))
    }
    #[track_caller]
    fn selection(self, selector: Selector) -> TextSelectionHandle {
        let query = Arc::new(Query {
            text: self.id,
            selector,
        });
        let location = Location::caller();
        TextSelectionHandle { query, location }
    }
}
impl<T, F: Field<TextSelection, T>> SelectionProperty<T, F> {
    fn resolve<M: Model2d>(
        &self,
        context: &mut SceneContext<'_, M>,
    ) -> Result<
        Property<TextSelectionId, TextSelection, T>,
        SelectionTargetError,
    > {
        if let Some(id) =
            context.cache::<Selections>().0.get(&self.query).copied()
        {
            return Ok(F::property(id));
        }
        let graphics = context.model().graphics();
        let text = graphics.texts.get(self.query.text.index()).ok_or_else(|| {
            SelectionTargetError {
                description: format!("text selection {:?}: text object does not exist ({})", self.query, self.location),
                cause: None,
            }
        })?;
        let result = match &self.query.selector {
            Selector::Literal { needle, occurrence } => {
                TextRange::find(
                    text.content.clone(),
                    needle,
                    *occurrence,
                )
            }
            Selector::Range(range) => {
                TextRange::new(text.content.clone(), range.clone())
            }
        };
        let range = result.map_err(|error| SelectionTargetError {
            description: format!(
                "text selection {:?} in {:?}: {} ({})",
                self.query.selector,
                text.content,
                error,
                self.location
            ),
            cause: Some(error),
        })?;
        // Different queries that resolve to the same source range share one target.
        let existing = text.selections.iter().copied().find(|id| {
            graphics.text_selections[id.index()].range == range
        });
        let id = if let Some(id) = existing {
            id
        } else {
            let graphics = context.model_mut().graphics_mut();
            let id = TextSelectionId(graphics.text_selections.len());
            graphics.text_selections.push(TextSelection {
                range,
                fill: graphics.texts[self.query.text.index()].fill,
                opacity: 1.0,
            });
            graphics.texts[self.query.text.index()]
                .selections
                .push(id);
            id
        };
        context
            .cache::<Selections>()
            .0
            .insert(self.query.clone(), id);
        Ok(F::property(id))
    }
    /// Animate to a target calculated from the value at the change's start.
    #[track_caller]
    pub fn to_with<M, A>(
        &self,
        target: A,
    ) -> motiongfx_authoring::Animation<M>
    where
        M: Model2d,
        T: Clone + ThreadSafe,
        A: Fn(&T) -> T + Clone + ThreadSafe,
    {
        motiongfx_authoring::Animation::from_step(SelectionStep {
            property: self.clone(),
            target,
        })
    }
    /// Animate to `value`.
    #[track_caller]
    pub fn to<M: Model2d>(
        self,
        value: impl Into<T>,
    ) -> motiongfx_authoring::Animation<M>
    where
        T: Clone + ThreadSafe,
    {
        let value = value.into();
        self.to_with(move |_| value.clone())
    }
    /// Replace the value instantly at the point where this is scheduled.
    #[track_caller]
    pub fn set<M: Model2d>(
        self,
        value: impl Into<T>,
    ) -> motiongfx_authoring::Animation<M>
    where
        T: Clone + ThreadSafe,
    {
        self.to(value).over(Duration::ZERO)
    }
    /// Animate by adding `delta` to the value at the change's start.
    #[track_caller]
    pub fn offset<M: Model2d, D: Copy + ThreadSafe>(
        self,
        delta: D,
    ) -> motiongfx_authoring::Animation<M>
    where
        T: Clone + ThreadSafe + std::ops::Add<D, Output = T>,
    {
        self.to_with(move |previous: &T| previous.clone() + delta)
    }
    /// Animate by multiplying the value at the change's start by `factor`.
    #[track_caller]
    pub fn multiply<M: Model2d, D: Copy + ThreadSafe>(
        self,
        factor: D,
    ) -> motiongfx_authoring::Animation<M>
    where
        T: Clone + ThreadSafe + std::ops::Mul<D, Output = T>,
    {
        self.to_with(move |previous: &T| previous.clone() * factor)
    }
}

/// A text selection that could not be resolved when the scene compiled.
/// The message includes the [`SelectionError`], which [`Self::cause`]
/// returns for inspection.
#[derive(Debug)]
pub struct SelectionTargetError {
    description: String,
    cause: Option<SelectionError>,
}
impl SelectionTargetError {
    /// Why the selection didn't match, or `None` if its text object is
    /// not in this scene.
    pub fn cause(&self) -> Option<&SelectionError> {
        self.cause.as_ref()
    }
}
impl std::fmt::Display for SelectionTargetError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str(&self.description)
    }
}
impl std::error::Error for SelectionTargetError {}

/// A selection change. The selection resolves against the text content
/// when the scene compiles; failures are reported with this location.
struct SelectionStep<T, F, A> {
    property: SelectionProperty<T, F>,
    target: A,
}
impl<M, T, F, A> Step<M> for SelectionStep<T, F, A>
where
    M: Model2d,
    T: Clone + ThreadSafe,
    F: Field<TextSelection, T>,
    A: Fn(&T) -> T + Clone + ThreadSafe,
{
    fn lower(
        &self,
        context: &mut SceneContext<'_, M>,
        timing: &Timing,
    ) -> TrackFragment {
        match self.property.resolve(context) {
            Ok(property) => lower_change(
                context,
                property,
                self.target.clone(),
                timing,
            ),
            Err(error) => {
                context.report(AnimationError {
                    location: timing.location(),
                    kind: AnimationErrorKind::Target(Arc::new(error)),
                });
                TrackFragment::silent(Duration::ZERO)
            }
        }
    }
}
impl<M: Model2d> SubjectSource<TextSelectionId, TextSelection>
    for World<M>
{
    fn get_source(
        &self,
        id: TextSelectionId,
    ) -> Option<&TextSelection> {
        self.model().graphics().text_selections.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: TextSelectionId,
        f: impl FnOnce(&mut TextSelection) -> R,
    ) -> Option<R> {
        self.model_mut()
            .graphics_mut()
            .text_selections
            .get_mut(id.0)
            .map(f)
    }
}

// Fixed field identity is generated locally; no accessor metadata is stored in
// each handle. See properties.rs and motiongfx_authoring::Target.
crate::properties::fields!(fields, TextSelection, TextSelectionId;
fill: Color => new(TextSelection.fill);
opacity: f64 => new(TextSelection.opacity);
);
