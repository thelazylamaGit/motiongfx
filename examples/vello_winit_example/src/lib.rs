//! Compatibility exports for the examples; the reusable hosts live in their
//! own crates.
//!
//! Examples open [`Window`]: the winit preview by default, or the zgui
//! editor with the `zgui` feature
//! (`cargo run --example sorting --features zgui`). Examples built on
//! `VelloWinitApp` always use the winit preview.
pub use vello_winit_motiongfx::*;

#[cfg(feature = "zgui")]
pub use zgui_editor::Window;

/// What a failed [`Window::run`] returns, for the host this build uses.
#[cfg(not(feature = "zgui"))]
pub type RunError = winit::error::EventLoopError;

/// What a failed [`Window::run`] returns, for the host this build uses.
#[cfg(feature = "zgui")]
pub type RunError = zgui_editor::Error;
