//! Text authoring state, independent of the temporary lettering renderer.

use crate::{Color, Parent, Point, Position, Scene, SceneObject};
use motiongfx_authoring::Property;
use std::sync::Arc;

/// Horizontal alignment relative to the text object's position.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Plain text with its own position, style and transform. Position is the top
/// of the text block; horizontal anchoring is controlled by `align`.
/// The initial renderer uses limited ASCII vector lettering, not font shaping.
#[derive(Clone)]
pub struct Text {
    pub content: Arc<str>,
    pub position: Point,
    pub font_size: f64,
    pub fill: Color,
    pub opacity: f64,
    pub scale: f64,
    pub rotation: f64,
    pub align: TextAlign,
    pub(crate) parent: Option<Parent>,
}

impl Text {
    pub fn new(
        content: impl Into<crate::Input<Arc<str>>>,
    ) -> crate::Object<Self> {
        crate::Object::new(Self {
            content: Arc::from(""),
            position: Point::ZERO,
            font_size: 24.0,
            fill: Color::WHITE,
            opacity: 1.0,
            scale: 1.0,
            rotation: 0.0,
            align: TextAlign::Left,
            parent: None,
        })
        .content(content)
    }
}

crate::object::properties!(Text;
    content: Arc<str>, position: Point, scale: f64, rotation: f64;
    style TextStyle {
        font_size: f64, fill: Color, opacity: f64, align: TextAlign,
    }
);
impl crate::Object<Text> {
    /// Make position local to a rectangle or circle. Its translation, rotation,
    /// and scale are inherited; text style and local animations stay independent.
    /// The parent must belong to the same scene as this text.
    pub fn relative_to(mut self, parent: impl Into<Parent>) -> Self {
        self.parent = Some(parent.into());
        self
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct TextId(pub(crate) usize);

/// Typed targets. Content is replaced discretely; numeric/style properties
/// interpolate through the existing MotionGfx machinery.
#[derive(Clone, Copy)]
pub struct TextHandle {
    pub(crate) id: TextId,
    pub content: Property<TextId, Text, Arc<str>>,
    pub position: Position<TextId, Text>,
    pub font_size: Property<TextId, Text, f64>,
    pub fill: Property<TextId, Text, Color>,
    pub opacity: Property<TextId, Text, f64>,
    pub scale: Property<TextId, Text, f64>,
    pub rotation: Property<TextId, Text, f64>,
    pub align: Property<TextId, Text, TextAlign>,
}

impl SceneObject for Text {
    type Handle = TextHandle;
    fn insert_into(self, scene: &mut Scene) -> Self::Handle {
        scene.insert_text(self)
    }
}
