//! Geometry shared by [`Triangle`](crate::Triangle) and
//! [`Chevron`](crate::Chevron): the same pointed shape, filled or as an
//! open outline.

use crate::Point;
use kurbo::Affine;

/// Where a shape at `position`, turned by `rotation` and scaled by `scale`,
/// sits on the canvas (y up, so y is flipped for the renderer).
pub(crate) fn transform(
    position: Point,
    rotation: f64,
    scale: f64,
) -> Affine {
    Affine::translate((position.x, -position.y))
        * Affine::rotate(-rotation)
        * Affine::scale(scale)
}

pub(crate) fn vertices(size: f64, angle: f64) -> [Point; 3] {
    assert!(
        size.is_finite() && size >= 0.0,
        "shape size must be finite and nonnegative"
    );
    assert!(
        angle.is_finite()
            && angle > 0.0
            && angle < std::f64::consts::PI,
        "opening angle must be between zero and pi radians"
    );
    let half_width = size * (angle / 2.0).tan();
    [
        Point::ZERO,
        Point::new(-size, half_width),
        Point::new(-size, -half_width),
    ]
}
