//! 2D objects (rectangles, circles, lines, paths, text, triangles and
//! chevrons), their styles and the camera, for authoring with
//! `motiongfx_authoring`. Renderers such as `vello_motiongfx` draw the result.
//!
//! Import [`prelude`] for the common types. Objects follow signals and
//! computed values directly:
//!
//! ```
//! use motiongfx::prelude::{ease, ms};
//! use motiongfx_2d::prelude::*;
//! let radius = signal(10.0);
//! let mut scene = Scene::new();
//! let circle = scene.add(Circle::new(&radius)
//!     .position(radius.map(|r| (r * 2.0, 0.0))));
//! let mut animation = scene.compile([radius.to(20.0).ease(ease::linear)]);
//! animation.sample(ms(500));
//! assert_eq!(animation.get(circle).radius, 15.0);
//! assert_eq!(animation.get(circle).position, Point::new(30.0, 0.0));
//! ```
#![warn(missing_docs)]
pub use kurbo::{Affine, Cap, Join, Point, Rect, Size, Vec2};
pub use motiongfx_authoring::{
    AnimationError, AnimationErrorKind, Background, BackgroundId,
    BackgroundProperty, BindingError, BindingSource, CompileError,
    ObjectHandle, SampleTime, SceneObject, Signal, SignalId,
    Simulated, Source, Spring, SpringCurve, SpringError, World, all,
    any, chain, computed, flow, marker, signal, smooth, wait,
};
pub use motiongfx_text::{
    FontStyle, FontWeight, SelectionError, TextRange,
};
pub use peniko::Color;
use peniko_motiongfx as _;
/// A 2D scene being built: add objects, then `compile` it with its
/// animations.
///
/// ```
/// use motiongfx::prelude::ms;
/// use motiongfx_2d::prelude::*;
///
/// let mut scene = Scene::new();
/// let dot = scene.add(Circle::new(10.0));
/// let mut animation = scene.compile([
///     dot.position_x().to(100.0).over(ms(500)),
///     dot.fill_to(motiongfx_2d::colors::ORANGE),
/// ]);
/// animation.sample(ms(250));
/// ```
pub type Scene = motiongfx_authoring::Scene<Canvas>;
/// One animation in a 2D scene: a change, group or wait. Pass a list of
/// them to `scene.compile(..)`, or combine them with `chain`/`all`/`any`/`flow`.
pub type Animation = motiongfx_authoring::Animation<Canvas>;
/// The result of `scene.compile(..)`: the scene with its baked timeline,
/// ready to sample at any time.
pub type CompiledScene = motiongfx_authoring::CompiledScene<Canvas>;
/// A value calculated from signals or other values in a 2D scene.
pub type Computed<T, M = Canvas> =
    motiongfx_authoring::Computed<World<M>, T>;

/// A model that contains 2D objects. [`Canvas`] is one; a model that
/// combines 2D with other kinds of objects implements this to expose its
/// `Canvas`, and can then use every 2D object.
pub trait Model2d: 'static {
    /// The 2D objects.
    fn canvas(&self) -> &Canvas;
    /// The 2D objects, mutably.
    fn canvas_mut(&mut self) -> &mut Canvas;
}
impl Model2d for Canvas {
    fn canvas(&self) -> &Canvas {
        self
    }
    fn canvas_mut(&mut self) -> &mut Canvas {
        self
    }
}
mod background;
mod camera;
mod capability;
mod handle;
pub use handle::{Handle, Object2d};
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
pub use camera::{Camera, CameraId};
pub use capability::{Fadable, Fillable, Strokable, Transformable};
pub use chevron::{Chevron, ChevronId};
pub use circle::{Circle, CircleId};
pub use input::Input;
pub use line::{Line, LineId};
pub use line_layout::{LineLayout, TipPlacement};
pub use object::Object;
pub use parent::Parent;
pub use path::{BezPath, Path, PathId};
pub use style::ApplyStyle;
pub use text::{Text, TextAlign, TextId, TextStyle};
pub use text_selection::{
    SelectionProperty, SelectionTargetError, TextSelection,
    TextSelectionHandle, TextSelectionId,
};
pub use tips::{
    EndpointTip, LineTip, LineTipHandle, LineWithTips, NoTip,
};
pub use triangle::{Triangle, TriangleId};
mod wedge;
pub use rounded_triangle::Geometry;
/// The types most 2D scenes use: `use motiongfx_2d::prelude::*;`.
pub mod prelude {
    pub use crate::{
        Animation, Camera, Cap, Chevron, Circle, Color,
        CompiledScene, Fadable, Fillable, FontStyle, FontWeight,
        Handle, Join, Line, Model2d, Path, Point, Rectangle, Scene,
        Size, Spring, Strokable, Text, TextAlign, TextStyle,
        Transformable, Triangle, Vec2, all, any, chain, computed,
        flow, marker, signal, wait,
    };
}

mod canvas;
mod rectangle;
pub use canvas::{Canvas, DrawObject};
pub use rectangle::{Rectangle, RectangleId};
