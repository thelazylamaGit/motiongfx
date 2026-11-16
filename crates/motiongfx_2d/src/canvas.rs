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
