//! Lightweight paint targets for source selections, not independently drawn nodes.
use crate::*;
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
use std::ops::Range;

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
#[derive(Clone, Copy)]
pub struct TextSelectionHandle {
    pub fill: Property<TextSelectionId, TextSelection, Color>,
    pub opacity: Property<TextSelectionId, TextSelection, f64>,
}
impl TextHandle {
    /// Select the first literal occurrence in the initial content snapshot.
    /// The selection overrides that range's paint; later overlapping selections
    /// win. Font rendering expands partial ligatures to whole clusters.
    pub fn select<M: Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
        needle: &str,
    ) -> Result<TextSelectionHandle, SelectionError> {
        self.select_nth(scene, needle, 0)
    }
    pub fn select_nth<M: Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
        needle: &str,
        occurrence: usize,
    ) -> Result<TextSelectionHandle, SelectionError> {
        let text = &scene.model().graphics().texts[self.id.index()];
        let range = TextRange::find(
            text.content.clone(),
            needle,
            occurrence,
        )?;
        Ok(self.insert_selection(scene, range))
    }
    /// Explicit UTF-8 byte range; construction validates the boundaries.
    pub fn select_range<M: Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
        range: Range<usize>,
    ) -> Result<TextSelectionHandle, SelectionError> {
        let text = &scene.model().graphics().texts[self.id.index()];
        let range = TextRange::new(text.content.clone(), range)?;
        Ok(self.insert_selection(scene, range))
    }
    fn insert_selection<M: Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
        range: TextRange,
    ) -> TextSelectionHandle {
        let graphics = scene.model_mut().graphics_mut();
        let id = TextSelectionId(graphics.text_selections.len());
        graphics.text_selections.push(TextSelection {
            range,
            fill: graphics.texts[self.id.index()].fill,
            opacity: 1.0,
        });
        graphics.texts[self.id.index()].selections.push(id);
        TextSelectionHandle {
            fill: Property::new(id, path!(TextSelection.fill)),
            opacity: Property::new(id, path!(TextSelection.opacity)),
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
