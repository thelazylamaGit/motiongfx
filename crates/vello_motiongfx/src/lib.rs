//! Vello rendering for the portable MotionGfx 2D model.
//! Author scenes with motiongfx_2d; sample with motiongfx_authoring; render here.
#![doc = include_str!("../README.md")]
pub use motiongfx_2d::*;
pub mod lettering;
mod path_renderer;
mod shape_renderer;
mod text_renderer;
mod triangle_geometry;
#[cfg(test)]
use motiongfx::prelude::{
    Registry, TrackOrdering, ease, ms, path, s,
};
#[cfg(test)]
use path_renderer::PathGeometry;
pub use shape_renderer::Renderer;
#[cfg(test)]
use shape_renderer::*;
#[cfg(test)]
use std::time::Duration;
use triangle_geometry::CachedTriangle;
#[cfg(test)]
use vello::kurbo::Stroke;
#[cfg(test)]
use vello::peniko::Fill;
#[cfg(test)]
mod binding_tests;
#[cfg(test)]
mod camera_tests;
#[cfg(test)]
mod graphics_tests;
#[cfg(test)]
mod line_tests;
#[cfg(test)]
mod signal_tests;
#[cfg(test)]
mod style_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod update_tests;
