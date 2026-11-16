//! Read access to a [`Canvas`] for renderers.
use crate::*;
/// Read access for renderers. Each list is indexed by its object IDs'
/// `index()`.
impl Canvas {
    /// The camera.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }
    /// Every rectangle.
    pub fn rectangles(&self) -> &[Rectangle] {
        &self.rectangles
    }
    /// Every circle, including line tips.
    pub fn circles(&self) -> &[Circle] {
        &self.circles
    }
    /// Every triangle, including line tips.
    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }
    /// Every chevron, including line tips.
    pub fn chevrons(&self) -> &[Chevron] {
        &self.chevrons
    }
    /// Every line.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
    /// Every path.
    pub fn paths(&self) -> &[Path] {
        &self.paths
    }
    /// Every text object.
    pub fn texts(&self) -> &[Text] {
        &self.texts
    }
    /// Every text selection that some animation used.
    pub fn text_selections(&self) -> &[TextSelection] {
        &self.text_selections
    }
    /// The objects to draw, back to front. Line tips are drawn with their
    /// line, so they aren't listed.
    pub fn draw_order(&self) -> &[DrawObject] {
        &self.order
    }
    /// The transform `text` inherits from its parent (identity if none).
    pub fn text_parent_transform(&self, text: &Text) -> Affine {
        self.parent_transform(text.parent)
    }
    /// The transform `circle` inherits from its parent (identity if none).
    pub fn circle_parent_transform(&self, circle: &Circle) -> Affine {
        self.parent_transform(circle.parent)
    }
}
impl Rectangle {
    /// Where the rectangle's local coordinates sit on the canvas. Scale is
    /// applied to the size instead, so strokes keep their width.
    pub fn transform(&self) -> Affine {
        Affine::translate((self.position.x, -self.position.y))
            * Affine::rotate(-self.rotation)
    }
}
