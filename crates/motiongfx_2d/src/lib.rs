//! Portable 2D state and authoring, independent of its renderer.
//!
//! ```
//! use motiongfx::prelude::{ease, ms};
//! use motiongfx_2d::prelude::*;
//! let radius = signal(10.0);
//! let mut scene = Scene::new();
//! let circle = scene.add(Circle::new(&radius)
//!     .position(radius.map(|r| (r * 2.0, 0.0))));
//! let mut animation = scene.animate(|sequence| {
//!     sequence.play(radius.to(20.0).ease(ease::linear));
//! });
//! animation.sample(ms(500));
//! assert_eq!(animation.circle(circle).radius, 15.0);
//! assert_eq!(animation.circle(circle).position, Point::new(30.0, 0.0));
//! ```
pub use kurbo::{Affine, Cap, Join, Point, Rect, Size, Vec2};
pub use motiongfx_authoring::{
    AnimationError, AnimationErrorKind, BindingError, BindingSource,
    SampleTime, SceneError, SceneObject, Signal, SignalId, Source,
    Spring, SpringCurve, SpringError, World, all, animate, chain,
    computed, flow, marker, signal, smooth, wait,
};
pub use motiongfx_text::{
    FontStyle, FontWeight, SelectionError, TextRange,
};
pub use peniko::Color;
use peniko_motiongfx as _;
pub type Scene = motiongfx_authoring::Scene<Graphics>;
/// One animation in a 2D scene: a change, group or wait. Pass a list of
/// them to `scene.compile(..)`, or combine them with `chain`/`all`/`flow`.
pub type Animation = motiongfx_authoring::Animation<Graphics>;
/// The result of `scene.compile(..)`: the scene with its baked timeline,
/// ready to sample at any time.
pub type CompiledScene = motiongfx_authoring::CompiledScene<Graphics>;
pub type Sequence<'a> =
    motiongfx_authoring::SceneSequence<'a, Graphics>;
pub type Computed<T, M = Graphics> =
    motiongfx_authoring::Computed<World<M>, T>;

/// Static access to a 2D store. Composite models implement this alongside
/// their other stores; subject reads keep using direct typed vector access.
pub trait Model2d: 'static {
    fn graphics(&self) -> &Graphics;
    fn graphics_mut(&mut self) -> &mut Graphics;
}
impl Model2d for Graphics {
    fn graphics(&self) -> &Graphics {
        self
    }
    fn graphics_mut(&mut self) -> &mut Graphics {
        self
    }
}
mod access;
mod background;
mod camera;
mod chevron;
mod circle;
pub mod colors;
#[doc(hidden)]
pub mod fields;
mod input;
mod line;
mod line_layout;
mod object;
mod parent;
mod path;
mod properties;
mod rounded_triangle;
mod style;
mod text;
mod text_selection;
mod tips;
mod triangle;
mod updates;
pub use background::BackgroundId;
pub use camera::{Camera, CameraHandle, CameraId};
pub use chevron::{Chevron, ChevronHandle, ChevronId};
pub use circle::{Circle, CircleHandle, CircleId};
pub use input::Input;
pub use line::{Line, LineHandle, LineId};
pub use line_layout::{LineLayout, TipPlacement};
pub use object::Object;
pub use parent::Parent;
pub use path::{BezPath, Path, PathHandle, PathId};
pub use properties::Position;
pub use style::ApplyStyle;
pub use text::{Text, TextAlign, TextHandle, TextId, TextStyle};
pub use text_selection::{
    SelectionProperty, TextSelection, TextSelectionHandle,
    TextSelectionId,
};
pub use tips::{
    EndpointTip, LineTip, LineTipHandle, LineWithTips, NoTip,
};
pub use triangle::{Triangle, TriangleHandle, TriangleId};
mod wedge;
pub use access::{CompiledScene2dExt, Scene2dExt};
pub use rounded_triangle::Geometry;
pub mod prelude {
    pub use crate::{
        Animation, Camera, Cap, Chevron, Circle, Color,
        CompiledScene, CompiledScene2dExt, FontStyle, FontWeight,
        Join, Line, Model2d, Path, Point, Rectangle, Scene,
        Scene2dExt, Size, Spring, Text, TextAlign, TextStyle,
        Triangle, Vec2, all, chain, computed, flow, marker, signal,
        wait,
    };
}

mod graphics;
mod rectangle;
pub use graphics::{DrawObject, Graphics};
pub use rectangle::{Rectangle, RectangleHandle, RectangleId};
