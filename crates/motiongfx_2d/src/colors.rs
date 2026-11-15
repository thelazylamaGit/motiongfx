//! Colours: a small palette (`colors::ORANGE`) and constructors from hue
//! based colour spaces ([`hsl`], [`hsv`], [`oklch`]).
//!
//! Every constructor returns an ordinary sRGB [`Color`], so it works
//! anywhere a colour does. Add transparency with `.with_alpha(a)`, which
//! works on any colour, including the palette.
//!
//! To animate one channel, such as hue, keep that channel in a signal and
//! let the fill follow it:
//!
//! ```
//! use motiongfx::prelude::s;
//! use motiongfx_2d::{colors::oklch, prelude::*};
//!
//! let hue = signal(0.0);
//! let mut scene = Scene::new();
//! scene.add(
//!     Rectangle::new(100.0, 100.0)
//!         .fill(hue.map(|hue| oklch(0.72, 0.17, hue))),
//! );
//! // Once round the colour wheel; seeking works in both directions.
//! let animation = scene.compile([hue.to(360.0).over(s(2))]);
//! ```

use crate::Color;
use peniko::color::{AlphaColor, Hsl, Hwb, Oklch};

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

/// An opaque colour from HSL: `hue` in degrees (0 red, 120 green, 240
/// blue), `saturation` and `lightness` from 0 to 1.
///
/// Lightness 0.5 gives the purest hue; above it mixes in white, below it
/// black. Hues at the same lightness can look very different in
/// brightness; use [`oklch`] when that matters.
///
/// # Examples
///
/// ```
/// use motiongfx_2d::{Color, colors::hsl};
/// assert_eq!(hsl(0.0, 1.0, 0.5).to_rgba8(), Color::from_rgb8(255, 0, 0).to_rgba8());
/// let glass = hsl(200.0, 0.8, 0.5).with_alpha(0.4);
/// ```
pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Color {
    AlphaColor::<Hsl>::new([
        hue,
        saturation * 100.0,
        lightness * 100.0,
        1.0,
    ])
    .convert()
}

/// An opaque colour from HSV: `hue` in degrees (0 red, 120 green, 240
/// blue), `saturation` and `value` from 0 to 1.
///
/// Saturation 1 and value 1 give the purest hue; lower saturation mixes in
/// white, lower value darkens towards black.
///
/// # Examples
///
/// ```
/// use motiongfx_2d::{Color, colors::hsv};
/// assert_eq!(hsv(120.0, 1.0, 1.0).to_rgba8(), Color::from_rgb8(0, 255, 0).to_rgba8());
/// ```
pub fn hsv(hue: f32, saturation: f32, value: f32) -> Color {
    // The `color` crate has no HSV space, but HSV maps directly onto its
    // HWB (hue, whiteness, blackness).
    let whiteness = (1.0 - saturation) * value;
    let blackness = 1.0 - value;
    AlphaColor::<Hwb>::new([
        hue,
        whiteness * 100.0,
        blackness * 100.0,
        1.0,
    ])
    .convert()
}

/// An opaque colour from Oklch: perceptual `lightness` from 0 to 1,
/// `chroma` (colourfulness, 0 for grey, rarely above 0.37) and `hue` in
/// degrees (about 30 red, 140 green, 265 blue).
///
/// Colours with the same lightness look equally bright at every hue, which
/// makes Oklch the best choice for gradients and colour wheels. Some
/// lightness and chroma pairs fall outside what a screen can show; they
/// are clipped when drawn, so keep chroma around 0.1 to 0.2 for even
/// results.
///
/// # Examples
///
/// ```
/// use motiongfx_2d::colors::oklch;
/// // A rainbow of evenly bright colours.
/// let rainbow: Vec<_> = (0..12)
///     .map(|i| oklch(0.72, 0.15, i as f32 * 30.0))
///     .collect();
/// ```
pub fn oklch(lightness: f32, chroma: f32, hue: f32) -> Color {
    AlphaColor::<Oklch>::new([lightness, chroma, hue, 1.0]).convert()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb8(color: Color) -> [u8; 3] {
        let [r, g, b, _] = color.to_rgba8().to_u8_array();
        [r, g, b]
    }

    #[test]
    fn hsl_takes_fractions_for_saturation_and_lightness() {
        assert_eq!(rgb8(hsl(0.0, 1.0, 0.5)), [255, 0, 0]);
        assert_eq!(rgb8(hsl(240.0, 1.0, 0.25)), [0, 0, 128]);
        assert_eq!(rgb8(hsl(90.0, 0.0, 1.0)), [255, 255, 255]);
    }

    #[test]
    fn hsv_matches_the_standard_conversion() {
        assert_eq!(rgb8(hsv(0.0, 1.0, 1.0)), [255, 0, 0]);
        assert_eq!(rgb8(hsv(0.0, 0.0, 1.0)), [255, 255, 255]);
        assert_eq!(rgb8(hsv(0.0, 1.0, 0.0)), [0, 0, 0]);
        // 50% saturation at 50% value: half the max channel is white.
        assert_eq!(rgb8(hsv(240.0, 0.5, 0.5)), [64, 64, 128]);
    }

    #[test]
    fn oklch_reaches_known_srgb_colours() {
        // CSS Color 4's Oklch coordinates for pure sRGB red and white.
        assert_eq!(
            rgb8(oklch(0.627_955, 0.257_683, 29.233_885)),
            [255, 0, 0]
        );
        assert_eq!(rgb8(oklch(1.0, 0.0, 0.0)), [255, 255, 255]);
    }
}
