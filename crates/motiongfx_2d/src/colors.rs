//! Small shared palette for the comparison examples.
//! Import individual constants with `use vello_motiongfx::colors::BLUE`.

use crate::Color;

pub const BLUE: Color = Color::from_rgb8(88, 196, 221);
/// Canvas Commons' default blue fallback (#89b4fa).
pub const PASTEL_BLUE: Color = Color::from_rgb8(137, 180, 250);
/// Canvas Commons' default mauve fallback (#cba6f7).
pub const MAUVE: Color = Color::from_rgb8(203, 166, 247);
pub const ORANGE: Color = Color::from_rgb8(255, 134, 47);
pub const YELLOW: Color = Color::from_rgb8(255, 255, 0);
pub const RED: Color = Color::from_rgb8(252, 98, 85);
pub const GREEN: Color = Color::from_rgb8(131, 193, 103);
pub const WHITE: Color = Color::WHITE;
pub const BLACK: Color = Color::BLACK;
pub const TRANSPARENT: Color = Color::TRANSPARENT;
