//! [`Transform`]: where an object sits on the canvas.
use crate::Point;
use kurbo::Affine;

/// Where an object sits: its position, uniform scale and rotation. Shapes,
/// paths and text hold one as their `transform` field; their builders and
/// handles still set and animate each part directly (`.position(..)`,
/// `handle.rotation()`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Position in canvas units (y up). What it marks, such as a centre or
    /// an apex, depends on the object.
    pub position: Point,
    /// Uniform scale; 1 is normal size.
    pub scale: f64,
    /// Rotation in radians, counterclockwise.
    pub rotation: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Point::ZERO,
            scale: 1.0,
            rotation: 0.0,
        }
    }
}

impl Transform {
    /// The canvas transform: translate, rotate, then scale. The canvas's y
    /// points up, so y is flipped for renderers whose y points down.
    pub fn affine(&self) -> Affine {
        self.without_scale() * Affine::scale(self.scale)
    }
    /// The canvas transform without the scale, for objects that apply scale
    /// to their geometry instead so strokes keep their width.
    pub fn without_scale(&self) -> Affine {
        Affine::translate((self.position.x, -self.position.y))
            * Affine::rotate(-self.rotation)
    }
}
