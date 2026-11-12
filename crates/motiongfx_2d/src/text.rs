//! Text authoring state, independent of the temporary lettering renderer.

use crate::{Color, Parent, Point, Position, SceneObject};
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
    pub fn new<M: crate::Model2d>(
        content: impl Into<crate::Input<Arc<str>, M>>,
    ) -> crate::Object<Self, M> {
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
        fill: Property::new(id, path!(Text.fill)),
        opacity: Property::new(id, path!(Text.opacity)),
        scale: Property::new(id, path!(Text.scale)),
        rotation: Property::new(id, path!(Text.rotation)),
        align: Property::discrete(id, path!(Text.align)),
    }
}
