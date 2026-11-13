//! Deferred, source-aware paint targets. Registration is construction work only.
use crate::*;
use motiongfx::prelude::{SubjectSource, ThreadSafe, TrackFragment};
use motiongfx_authoring::{
    AnimationChange, Field, Property, SceneContext, Sequence,
    SpringValue,
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
    pub fn fill_to(
        &self,
        color: impl Into<Color>,
    ) -> SelectionChange<
        Color,
        fields::fill::Marker,
        impl Fn(&Color) -> Color + ThreadSafe,
    > {
        self.fill().to(color)
    }
    pub fn set_fill(
        &self,
        color: impl Into<Color>,
    ) -> SelectionChange<
        Color,
        fields::fill::Marker,
        impl Fn(&Color) -> Color + ThreadSafe,
    > {
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
    ) -> Result<Property<TextSelectionId, TextSelection, T>, String>
    {
        if let Some(id) =
            context.cache::<Selections>().0.get(&self.query).copied()
        {
            return Ok(F::property(id));
        }
        let graphics = context.model().graphics();
        let text = graphics.texts.get(self.query.text.index()).ok_or_else(|| {
            format!("text selection {:?}: text object does not exist ({})", self.query, self.location)
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
        let range = result.map_err(|error| {
            format!(
                "text selection {:?} in {:?}: {} ({})",
                self.query.selector,
                text.content,
                error,
                self.location
            )
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
    pub fn map_target<A: Fn(&T) -> T + ThreadSafe>(
        &self,
        target: A,
    ) -> SelectionChange<T, F, A> {
        SelectionChange {
            property: self.clone(),
            target,
            duration: None,
            ease: None,
        }
    }
    pub fn to(
        self,
        value: impl Into<T>,
    ) -> SelectionChange<T, F, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + ThreadSafe,
    {
        let value = value.into();
        self.map_target(move |_| value.clone())
    }
    pub fn set(
        self,
        value: impl Into<T>,
    ) -> SelectionChange<T, F, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + ThreadSafe,
    {
        self.to(value).over(Duration::ZERO)
    }
    pub fn offset<D: Copy + ThreadSafe>(
        self,
        delta: D,
    ) -> SelectionChange<T, F, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + std::ops::Add<D, Output = T>,
    {
        self.map_target(move |previous| previous.clone() + delta)
    }
    pub fn multiply<D: Copy + ThreadSafe>(
        self,
        factor: D,
    ) -> SelectionChange<T, F, impl Fn(&T) -> T + ThreadSafe>
    where
        T: Clone + std::ops::Mul<D, Output = T>,
    {
        self.map_target(move |previous| previous.clone() * factor)
    }
}
#[must_use = "schedule the selection change with sequence.play"]
pub struct SelectionChange<T, F, A> {
    property: SelectionProperty<T, F>,
    target: A,
    duration: Option<Duration>,
    ease: Option<motiongfx::prelude::EaseFn>,
}
impl<T, F, A> SelectionChange<T, F, A> {
    pub fn spring<M>(
        self,
        spring: impl Into<Spring>,
    ) -> SelectionSpringChange<T, F, A, M>
    where
        T: SpringValue<M>,
    {
        SelectionSpringChange {
            change: self,
            spring: spring.into(),
            marker: PhantomData,
        }
    }
    pub fn over(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
    pub fn ease(mut self, ease: motiongfx::prelude::EaseFn) -> Self {
        self.ease = Some(ease);
        self
    }
}
impl<M, T, F, A> AnimationChange<World<M>, SceneContext<'_, M>>
    for SelectionChange<T, F, A>
where
    M: Model2d,
    T: Clone + ThreadSafe,
    F: Field<TextSelection, T>,
    A: Fn(&T) -> T + ThreadSafe,
{
    fn lower(
        self,
        sequence: &mut Sequence<'_, World<M>, SceneContext<'_, M>>,
    ) -> TrackFragment {
        match self.property.resolve(sequence.context_mut()) {
            Ok(property) => {
                let mut change = property.map_target(self.target);
                if let Some(duration) = self.duration {
                    change = change.over(duration);
                }
                if let Some(ease) = self.ease {
                    change = change.ease(ease);
                }
                change.lower(sequence)
            }
            Err(error) => {
                sequence.context_mut().reject(error);
                TrackFragment::silent(Duration::ZERO)
            }
        }
    }
}

#[must_use = "schedule the selection spring with sequence.play"]
pub struct SelectionSpringChange<T, F, A, M> {
    change: SelectionChange<T, F, A>,
    spring: Spring,
    marker: PhantomData<fn() -> M>,
}
impl<Model, T, F, A, M>
    AnimationChange<World<Model>, SceneContext<'_, Model>>
    for SelectionSpringChange<T, F, A, M>
where
    Model: Model2d,
    T: SpringValue<M> + Clone + ThreadSafe,
    F: Field<TextSelection, T>,
    A: Fn(&T) -> T + ThreadSafe,
{
    fn lower(
        self,
        sequence: &mut Sequence<
            '_,
            World<Model>,
            SceneContext<'_, Model>,
        >,
    ) -> TrackFragment {
        match self.change.property.resolve(sequence.context_mut()) {
            Ok(property) => {
                let mut change =
                    property.map_target(self.change.target);
                if let Some(duration) = self.change.duration {
                    change = change.over(duration);
                }
                if let Some(ease) = self.change.ease {
                    change = change.ease(ease);
                }
                change.spring::<M>(self.spring).lower(sequence)
            }
            Err(error) => {
                sequence.context_mut().reject(error);
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
