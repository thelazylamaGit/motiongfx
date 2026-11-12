//! Scene background state, consumed as the renderer's clear/base color.

use crate::Color;
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;

pub(crate) struct Background(pub Color);
impl Default for Background {
    fn default() -> Self {
        Self(Color::from_rgb8(28, 28, 46))
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct BackgroundId;

pub(crate) fn property() -> Property<BackgroundId, Color, Color> {
    Property::new(BackgroundId, path!(Color))
}

impl<M: crate::Model2d> SubjectSource<BackgroundId, Color>
    for crate::World<M>
{
    fn get_source(&self, _: BackgroundId) -> Option<&Color> {
        Some(&self.model().graphics().background.0)
    }
    fn apply_source<R>(
        &mut self,
        _: BackgroundId,
        f: impl FnOnce(&mut Color) -> R,
    ) -> Option<R> {
        Some(f(&mut self.model_mut().graphics_mut().background.0))
    }
}
