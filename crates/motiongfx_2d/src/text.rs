//! Portable text state, independent of renderer-owned glyph/layout caches.

use crate::{
    Color, FontStyle, FontWeight, Parent, Point, Position,
    SceneObject, TextSelectionId,
};
use crate::{DrawObject, Model2d, World};
use motiongfx::prelude::{SubjectSource, path};
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
/// Defaults to embedded Fira Sans. Family selection and layout are shared across renderers.
#[derive(Clone)]
pub struct Text {
    pub content: Arc<str>,
    pub position: Point,
    pub font_size: f64,
    pub font_family: Arc<str>,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub line_height: f64,
    pub letter_spacing: f64,
    pub wrap_width: f64,
    pub fill: Color,
    pub opacity: f64,
    pub scale: f64,
    pub rotation: f64,
    pub align: TextAlign,
    pub(crate) parent: Option<Parent>,
    pub(crate) selections: Vec<TextSelectionId>,
}

impl Text {
    pub fn new<M: crate::Model2d>(
        content: impl Into<crate::Input<Arc<str>, M>>,
    ) -> crate::Object<Self, M> {
        crate::Object::new(Self {
            content: Arc::from(""),
            position: Point::ZERO,
            font_size: 24.0,
            font_family: "Fira Sans".into(),
            font_weight: FontWeight::NORMAL,
            font_style: FontStyle::Normal,
            line_height: 1.3,
            letter_spacing: 0.0,
            wrap_width: 0.0,
            fill: Color::WHITE,
            opacity: 1.0,
            scale: 1.0,
            rotation: 0.0,
            align: TextAlign::Left,
            parent: None,
            selections: Vec::new(),
        })
        .content(content)
    }
    pub fn selections(&self) -> &[TextSelectionId] {
        &self.selections
    }
}

crate::object::properties!(Text;
    content: Arc<str>, position: Point, scale: f64, rotation: f64;
    style TextStyle {
        font_size: f64, fill: Color, opacity: f64, align: TextAlign,
        font_family: Arc<str>, font_weight: FontWeight, font_style: FontStyle,
        line_height: f64, letter_spacing: f64, wrap_width: f64,
    }
);
impl<M: crate::Model2d> crate::Object<Text, M> {
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
impl TextId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// Typed targets. Content is replaced discretely; numeric/style properties
/// interpolate through the existing MotionGfx machinery.
#[derive(Clone, Copy)]
pub struct TextHandle {
    pub(crate) id: TextId,
    pub content: Property<TextId, Text, Arc<str>>,
    pub position: Position<TextId, Text>,
    pub font_size: Property<TextId, Text, f64>,
    pub font_family: Property<TextId, Text, Arc<str>>,
    pub font_weight: Property<TextId, Text, FontWeight>,
    pub font_style: Property<TextId, Text, FontStyle>,
    pub line_height: Property<TextId, Text, f64>,
    pub letter_spacing: Property<TextId, Text, f64>,
    pub wrap_width: Property<TextId, Text, f64>,
    pub fill: Property<TextId, Text, Color>,
    pub opacity: Property<TextId, Text, f64>,
    pub scale: Property<TextId, Text, f64>,
    pub rotation: Property<TextId, Text, f64>,
    pub align: Property<TextId, Text, TextAlign>,
}
impl TextHandle {
    pub fn id(self) -> TextId {
        self.id
    }
}

impl<M: crate::Model2d> SceneObject<M> for Text {
    type Handle = TextHandle;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Self::Handle {
        insert_text(scene, self)
    }
}

impl<M: Model2d> SubjectSource<TextId, Text> for World<M> {
    fn get_source(&self, id: TextId) -> Option<&Text> {
        self.model().graphics().texts.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: TextId,
        f: impl FnOnce(&mut Text) -> R,
    ) -> Option<R> {
        self.model_mut().graphics_mut().texts.get_mut(id.0).map(f)
    }
}

pub(crate) fn insert_text<M: Model2d>(
    scene: &mut motiongfx_authoring::Scene<M>,
    text: Text,
) -> TextHandle {
    let id = TextId(scene.model_mut().graphics_mut().texts.len());
    scene.model_mut().graphics_mut().texts.push(text);
    scene
        .model_mut()
        .graphics_mut()
        .order
        .push(DrawObject::Text(id));
    TextHandle {
        id,
        content: Property::discrete(id, path!(Text.content)),
        position: Position::new(
            id,
            path!(Text.position),
            path!(Text.position.x),
            path!(Text.position.y),
        ),
        font_size: Property::new(id, path!(Text.font_size)),
        font_family: Property::discrete(id, path!(Text.font_family)),
        font_weight: Property::new(id, path!(Text.font_weight)),
        font_style: Property::discrete(id, path!(Text.font_style)),
        line_height: Property::new(id, path!(Text.line_height)),
        letter_spacing: Property::new(id, path!(Text.letter_spacing)),
        wrap_width: Property::new(id, path!(Text.wrap_width)),
        fill: Property::new(id, path!(Text.fill)),
        opacity: Property::new(id, path!(Text.opacity)),
        scale: Property::new(id, path!(Text.scale)),
        rotation: Property::new(id, path!(Text.rotation)),
        align: Property::discrete(id, path!(Text.align)),
    }
}
