//! The canvas's background colour, which renderers clear each frame with.
//! Set it with `scene.set_background(..)`.

use crate::{Canvas, Color};
use motiongfx::prelude::Interpolation;
use peniko_motiongfx::Peniko;

/// Stores the colour with its default.
pub(crate) struct BackgroundColor(pub(crate) Color);
impl Default for BackgroundColor {
    fn default() -> Self {
        Self(Color::from_rgb8(28, 28, 46))
    }
}

impl motiongfx_authoring::Background for Canvas {
    type Color = Color;
    fn background(&self) -> &Color {
        &self.background.0
    }
    fn background_mut(&mut self) -> &mut Color {
        &mut self.background.0
    }
    fn blend(start: &Color, end: &Color, t: f32) -> Color {
        <Color as Interpolation<Peniko>>::interp(start, end, t)
    }
}
