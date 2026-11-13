//! Portable text state, independent of renderer-owned glyph/layout caches.

use crate::{
    Color, FontStyle, FontWeight, Parent, Point, SceneObject,
    TextSelectionId,
};
use crate::{DrawObject, Model2d, World};
use motiongfx::prelude::SubjectSource;
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
    TextHandle { id }
}

// Fixed field identity is generated locally; no accessor metadata is stored in
// each handle. See properties.rs and motiongfx_authoring::Target.
crate::properties::fields!(fields, Text, TextId;
content: Arc<str> => discrete(Text.content);
font_size: f64 => new(Text.font_size);
font_family: Arc<str> => discrete(Text.font_family);
font_weight: FontWeight => new(Text.font_weight);
font_style: FontStyle => discrete(Text.font_style);
line_height: f64 => new(Text.line_height);
letter_spacing: f64 => new(Text.letter_spacing);
wrap_width: f64 => new(Text.wrap_width);
fill: Color => new(Text.fill);
opacity: f64 => new(Text.opacity);
scale: f64 => new(Text.scale);
rotation: f64 => new(Text.rotation);
align: TextAlign => discrete(Text.align);
position: Point => new(Text.position);
position_x: f64 => new(Text.position.x);
position_y: f64 => new(Text.position.y);
);
#[allow(non_camel_case_types)]
pub type text_position = crate::Position<
    TextId,
    Text,
    fields::position::Target,
    fields::position_x::Target,
    fields::position_y::Target,
>;
pub fn text_position_axes(id: TextId) -> text_position {
    crate::Position::from_targets(
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
    )
}

crate::properties::handle_methods!(TextHandle,Text,TextId;
content: Arc<str>;
font_size: f64;
font_family: Arc<str>;
font_weight: FontWeight;
font_style: FontStyle;
line_height: f64;
letter_spacing: f64;
wrap_width: f64;
fill: Color;
opacity: f64;
scale: f64;
rotation: f64;
align: TextAlign;
position: Point;
position_x: f64;
position_y: f64;
);

crate::properties::transform_methods!(TextHandle, Text, TextId);

crate::properties::fill_methods!(TextHandle, Text, TextId);

impl TextHandle {
    /// Replace plain text at the scheduled time, without a tween or duration.
    pub fn set_content(
        self,
        content: impl Into<Arc<str>>,
    ) -> motiongfx_authoring::Change<
        TextId,
        Text,
        Arc<str>,
        impl Fn(&Arc<str>) -> Arc<str> + Send + Sync,
    > {
        self.content().set(content)
    }
}
