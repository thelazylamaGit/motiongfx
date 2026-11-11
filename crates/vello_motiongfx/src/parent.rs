//! Transform-only anchoring between objects. A parent must already exist in
//! the same scene; authoring therefore constructs parent links before children.

use crate::{
    CircleHandle, CircleId, Graphics, RectangleHandle, RectangleId,
};
use vello::kurbo::Affine;

#[derive(Clone, Copy)]
pub enum Parent {
    Rectangle(RectangleId),
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

impl Graphics {
    pub(crate) fn parent_transform(
        &self,
        parent: Option<Parent>,
    ) -> Affine {
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
