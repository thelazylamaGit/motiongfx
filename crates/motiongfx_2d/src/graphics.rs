//! Typed, renderer-independent 2D storage in draw order.
use crate::*;
#[derive(Default)]
pub struct Graphics {
    pub(crate) background: background::Background,
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

#[derive(Clone, Copy)]
pub enum DrawObject {
    Triangle(TriangleId),
    Chevron(ChevronId),
    Rectangle(RectangleId),
    Circle(CircleId),
    Line(LineId),
    Path(PathId),
    Text(TextId),
}
