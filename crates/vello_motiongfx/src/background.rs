//! Scene background state, consumed as the renderer's clear/base color.

use crate::{Color, Graphics, Input, Scene};
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

fn property() -> Property<BackgroundId, Color, Color> {
    Property::new(BackgroundId, path!(Color))
}

impl Scene {
    /// Set the renderer's background, without adding a drawable. Accepts a
    /// fixed color, signal, or computed color. The last call wins. The returned
    /// property can be animated directly when configured with a fixed color.
    pub fn set_background(
        &mut self,
        color: impl Into<Input<Color>>,
    ) -> Property<BackgroundId, Color, Color> {
        let color = color.into();
        if let Input::Fixed(value) = &color {
            self.graphics.background.0 = *value;
        }
        self.background_input = Some(color);
        property()
    }

    pub(crate) fn prepare_background(&mut self) {
        if let Some(source) = self.background_input.take()
            && !matches!(source, Input::Fixed(_))
        {
            self.bind(property(), source, std::convert::identity);
        }
    }
}

impl crate::Animation {
    /// Current sampled clear color. Rendering hosts pass this to Vello's
    /// RenderParams::base_color; it is independent of camera/canvas fitting.
    pub fn background(&self) -> Color {
        self.graphics.background.0
    }
}

impl SubjectSource<BackgroundId, Color> for Graphics {
    fn get_source(&self, _: BackgroundId) -> Option<&Color> {
        Some(&self.background.0)
    }
    fn apply_source<R>(
        &mut self,
        _: BackgroundId,
        f: impl FnOnce(&mut Color) -> R,
    ) -> Option<R> {
        Some(f(&mut self.background.0))
    }
}
