//! [`Parent`]: an object that others attach to with `relative_to`, moving,
//! turning and scaling with it.

use crate::{
    Canvas, CircleHandle, CircleId, RectangleHandle, RectangleId,
};
use kurbo::Affine;

/// An object that others can attach to with `relative_to`. Pass a
/// rectangle or circle handle; it converts into this.
#[derive(Clone, Copy, Debug)]
pub enum Parent {
    /// A rectangle.
    Rectangle(RectangleId),
    /// A circle.
    Circle(CircleId),
}

impl From<RectangleHandle> for Parent {
    fn from(handle: RectangleHandle) -> Self {
        Self::Rectangle(handle.id)
    }
}
impl From<CircleHandle> for Parent {
    fn from(handle: CircleHandle) -> Self {
        Self::Circle(handle.id)
    }
}

impl Canvas {
    /// The transform a child of `parent` inherits, including the parent's
    /// own parents; identity for `None`.
    pub fn parent_transform(&self, parent: Option<Parent>) -> Affine {
        match parent {
            Some(Parent::Rectangle(id)) => {
                let rectangle = &self.rectangles[id.0];
                rectangle.transform() * Affine::scale(rectangle.scale)
            }
            Some(Parent::Circle(id)) => {
                let circle = &self.circles[id.0];
                self.parent_transform(circle.parent)
                    * circle.transform()
            }
            None => Affine::IDENTITY,
        }
    }
}
