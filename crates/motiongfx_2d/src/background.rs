//! The scene's background colour, which renderers clear the frame with.

use crate::Color;
use motiongfx::prelude::path;
use motiongfx_authoring::Property;

pub(crate) struct Background(pub Color);
impl Default for Background {
    fn default() -> Self {
        Self(Color::from_rgb8(28, 28, 46))
    }
}

/// Identifies the scene's background colour.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct BackgroundId;

pub(crate) fn property() -> Property<BackgroundId, Color, Color> {
    Property::new(BackgroundId, path!(Color))
}

impl<M: crate::Model2d>
    motiongfx_authoring::WorldObject<BackgroundId, M> for Color
{
    fn get(
        world: &crate::World<M>,
        _: BackgroundId,
    ) -> Option<&Self> {
        Some(&world.model().graphics().background.0)
    }
    fn get_mut(
        world: &mut crate::World<M>,
        _: BackgroundId,
    ) -> Option<&mut Self> {
        Some(&mut world.model_mut().graphics_mut().background.0)
    }
}
