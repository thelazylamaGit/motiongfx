//! A small palette used by the examples, e.g. `colors::ORANGE`.

use crate::Color;

/// Light blue (#58c4dd).
pub const BLUE: Color = Color::from_rgb8(88, 196, 221);
/// Canvas Commons' default blue fallback (#89b4fa).
pub const PASTEL_BLUE: Color = Color::from_rgb8(137, 180, 250);
/// Canvas Commons' default mauve fallback (#cba6f7).
pub const MAUVE: Color = Color::from_rgb8(203, 166, 247);
/// Orange (#ff862f).
pub const ORANGE: Color = Color::from_rgb8(255, 134, 47);
/// Yellow (#ffff00).
pub const YELLOW: Color = Color::from_rgb8(255, 255, 0);
/// Red (#fc6255).
pub const RED: Color = Color::from_rgb8(252, 98, 85);
/// Green (#83c167).
pub const GREEN: Color = Color::from_rgb8(131, 193, 103);
/// White.
pub const WHITE: Color = Color::WHITE;
/// Black.
pub const BLACK: Color = Color::BLACK;
/// Fully transparent, for no fill or no stroke.
pub const TRANSPARENT: Color = Color::TRANSPARENT;
