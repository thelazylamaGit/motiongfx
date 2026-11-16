//! [`Text`]: text objects, with real font shaping.
use crate::Handle;

use crate::{
    Color, FontStyle, FontWeight, Parent, Point, SceneObject,
    TextSelectionId,
};
use crate::{DrawObject, Model2d};
use std::sync::Arc;

/// Which side of a text block its position anchors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    /// The position is the block's left edge.
    #[default]
    Left,
    /// The position is the block's centre.
    Center,
    /// The position is the block's right edge.
    Right,
}

/// A block of text. Create one with [`Text::new`]. The font defaults to the
/// embedded Fira Sans; share settings between texts with a [`TextStyle`].
///
/// [`TextStyle`]: crate::TextStyle
#[derive(Clone)]
pub struct Text {
    /// The text shown.
    pub content: Arc<str>,
    /// Top of the text block, in canvas units (y up); `align` sets which side.
    pub position: Point,
    /// Font size in canvas units.
    pub font_size: f64,
    /// Font family name; Fira Sans is embedded by default.
    pub font_family: Arc<str>,
    /// Font weight, such as `FontWeight::BOLD`.
    pub font_weight: FontWeight,
    /// Normal, italic or oblique.
    pub font_style: FontStyle,
    /// Line height as a multiple of the font size.
    pub line_height: f64,
    /// Extra space between letters, in canvas units.
    pub letter_spacing: f64,
    /// Equal-width digits for counters and changing values; see `LayoutStyle`.
    pub tabular_numbers: bool,
    /// Width to wrap lines at, in canvas units; 0 for no wrapping.
    pub wrap_width: f64,
    /// Text colour.
    pub fill: Color,
    /// 0 (invisible) to 1 (opaque).
    pub opacity: f64,
    /// Uniform scale; 1 is normal size.
    pub scale: f64,
    /// Rotation in radians, counterclockwise.
    pub rotation: f64,
    /// Which side of the block `position` anchors.
    pub align: TextAlign,
    pub(crate) parent: Option<Parent>,
    pub(crate) selections: Vec<TextSelectionId>,
}

impl Text {
    /// White 24-unit text showing `content` (fixed, or a source to follow).
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
            tabular_numbers: false,
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
    /// The selections registered on this text, in registration order.
    pub fn selections(&self) -> &[TextSelectionId] {
        &self.selections
    }
}

crate::style::partial_style!(Text, TextStyle;
    font_size: f64, fill: Color, opacity: f64, align: TextAlign,
    font_family: Arc<str>, font_weight: FontWeight, font_style: FontStyle,
    line_height: f64, letter_spacing: f64, tabular_numbers: bool,
    wrap_width: f64,
);
impl<M: crate::Model2d> crate::Object<Text, M> {
    /// Attach to a rectangle or circle already in the scene: `position`
    /// becomes an offset from it, and the text moves, turns and scales with
    /// it. Style stays independent.
    pub fn relative_to(mut self, parent: impl Into<Parent>) -> Self {
        self.parent = Some(parent.into());
        self
    }
}

/// Identifies one [`Text`] in its scene.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct TextId(pub(crate) usize);
impl TextId {
    /// Its position in the scene's storage for this object type.
    pub fn index(self) -> usize {
        self.0
    }
}

impl<M: crate::Model2d> SceneObject<M> for Text {
    type Handle = Handle<Text>;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Self::Handle {
        insert_text(scene, self)
    }
}

crate::capability::stored_in!(Text, TextId, texts);

pub(crate) fn insert_text<M: Model2d>(
    scene: &mut motiongfx_authoring::Scene<M>,
    text: Text,
) -> Handle<Text> {
    let id = TextId(scene.model_mut().canvas_mut().texts.len());
    scene.model_mut().canvas_mut().texts.push(text);
    scene
        .model_mut()
        .canvas_mut()
        .order
        .push(DrawObject::Text(id));
    Handle::new(id)
}

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
crate::properties::object!(Text, TextId;
set content: Arc<str> => discrete(Text.content);
set position: Point => new(Text.position) shared;
set scale: f64 => new(Text.scale) shared;
set rotation: f64 => new(Text.rotation) shared;
set font_size: f64 => new(Text.font_size);
set font_family: Arc<str> => discrete(Text.font_family);
set font_weight: FontWeight => new(Text.font_weight);
set font_style: FontStyle => discrete(Text.font_style);
set line_height: f64 => new(Text.line_height);
set letter_spacing: f64 => new(Text.letter_spacing);
set tabular_numbers: bool => discrete(Text.tabular_numbers);
set wrap_width: f64 => new(Text.wrap_width);
set fill: Color => new(Text.fill) shared;
set opacity: f64 => new(Text.opacity) shared;
set align: TextAlign => discrete(Text.align);
get position_x: f64 => new(Text.position.x) shared;
get position_y: f64 => new(Text.position.y) shared;
);
crate::capability::capabilities!(Text; Transformable, Fillable, Fadable);

impl Handle<Text> {
    /// Change the text instantly.
    #[track_caller]
    pub fn set_content<M: crate::Model2d>(
        self,
        content: impl Into<Arc<str>>,
    ) -> motiongfx_authoring::Animation<M> {
        self.content().set(content)
    }
}
