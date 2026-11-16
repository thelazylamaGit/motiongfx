//! [`Canvas`]: the 2D model that stores a scene's objects.
use crate::*;
/// The 2D model: every object in a scene, by type, plus the camera,
/// background and draw order. `motiongfx_2d::Scene` uses it as its model;
/// renderers read it through its accessor methods.
#[derive(Default)]
pub struct Canvas {
    pub(crate) background: background::BackgroundColor,
    pub(crate) camera: Camera,
    pub(crate) rectangles: Vec<Rectangle>,
    pub(crate) circles: Vec<Circle>,
    pub(crate) triangles: Vec<Triangle>,
    pub(crate) chevrons: Vec<Chevron>,
    pub(crate) lines: Vec<Line>,
    pub(crate) paths: Vec<Path>,
    pub(crate) texts: Vec<Text>,
    pub(crate) text_selections: Vec<TextSelection>,
    pub(crate) order: Vec<DrawObject>,
}

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

/// One entry in [`Canvas::draw_order`]: which object to draw next.
#[derive(Clone, Copy, Debug)]
pub enum DrawObject {
    /// A triangle.
    Triangle(TriangleId),
    /// A chevron.
    Chevron(ChevronId),
    /// A rectangle.
    Rectangle(RectangleId),
    /// A circle.
    Circle(CircleId),
    /// A line, with its tips.
    Line(LineId),
    /// A path.
    Path(PathId),
    /// A text object.
    Text(TextId),
}
