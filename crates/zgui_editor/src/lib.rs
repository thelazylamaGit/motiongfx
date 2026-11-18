//! An editor window for MotionGfx animations, built with zgui: the scene
//! renders with Vello into the window's own GPU device, above a timeline
//! with play/pause and a scrubber.
//!
//! [`Window`] has the same builder as `vello_winit_motiongfx::Window`, so
//! an example can run in either host.
//!
//! ```no_run
//! use std::time::Duration;
//! use motiongfx_2d::prelude::*;
//! use zgui_editor::Window;
//!
//! let mut scene = Scene::new();
//! let dot = scene.add(Circle::new(20.0));
//! let animation = scene.compile([
//!     dot.position_x().to(200.0).over(Duration::from_millis(800)),
//! ]);
//! Window::new("Moving dot", (800.0, 450.0)).run(animation)?;
//! # Ok::<(), zgui_editor::Error>(())
//! ```
#![warn(missing_docs)]

mod editor;
mod preview;
mod timeline;

use vello::kurbo::Size;
pub use vello_winit_motiongfx::PlaybackMode;

/// What stopped the editor: a desktop that would not open a window, or a
/// machine with no usable graphics device.
pub type Error = zgui::Error;

/// An editor window for a compiled scene. Playback runs for the
/// animation's duration.
pub struct Window {
    title: &'static str,
    canvas: Size,
    playback_mode: PlaybackMode,
    renderer: vello_motiongfx::Renderer,
    #[cfg(feature = "audio")]
    audio: bool,
}

impl Window {
    /// A window titled `title` showing a `(width, height)` canvas, centred
    /// on the origin. The canvas is scaled to fit the preview.
    ///
    /// # Panics
    ///
    /// If the width or height isn't finite and positive.
    #[track_caller]
    pub fn new(
        title: &'static str,
        canvas: impl Into<(f64, f64)>,
    ) -> Self {
        let (width, height) = canvas.into();
        assert!(
            width.is_finite() && width > 0.0,
            "canvas width must be finite and positive, got {width}"
        );
        assert!(
            height.is_finite() && height > 0.0,
            "canvas height must be finite and positive, got {height}"
        );
        Self {
            title,
            canvas: Size::new(width, height),
            playback_mode: PlaybackMode::Once,
            renderer: vello_motiongfx::Renderer::default(),
            #[cfg(feature = "audio")]
            audio: false,
        }
    }

    /// Play the scene's sound events (notes and samples from
    /// `motiongfx_audio`) through the default output device. Sound plays
    /// while time moves forwards and stops on a pause, seek or reverse.
    /// If the device can't be opened, the editor runs silently and says
    /// why on stderr.
    #[cfg(feature = "audio")]
    pub fn audio(mut self) -> Self {
        self.audio = true;
        self
    }

    /// Play once (the default), loop, ping-pong, or loop after an intro.
    pub fn playback_mode(mut self, mode: PlaybackMode) -> Self {
        self.playback_mode = mode;
        self
    }

    /// Draw with `renderer`, for example one with extra fonts registered
    /// through `Renderer::fonts_mut()`.
    pub fn renderer(
        mut self,
        renderer: vello_motiongfx::Renderer,
    ) -> Self {
        self.renderer = renderer;
        self
    }

    /// Open the window and play `animation` until the window is closed.
    ///
    /// # Errors
    ///
    /// If no window can be opened or there is no usable graphics device.
    ///
    /// # Panics
    ///
    /// If the animation has no duration, or the Vello renderer can't be
    /// created on the window's device.
    pub fn run(
        self,
        animation: motiongfx_2d::CompiledScene,
    ) -> Result<(), Error> {
        let title = self.title;
        let (width, height) = (self.canvas.width, self.canvas.height);
        let mut setup = Some(editor::Setup {
            animation,
            renderer: self.renderer,
            canvas: self.canvas,
            playback_mode: self.playback_mode,
            #[cfg(feature = "audio")]
            audio: self.audio,
        });
        zgui::app()
            .with_title(title)
            .with_size(
                width as f32,
                (height + timeline::HEIGHT) as f32,
            )
            .with_stylesheet(editor::SHEET)
            .with_stylesheet(timeline::SHEET)
            // zgui builds the view once per window; this app opens one.
            .run(move || {
                editor::editor(
                    setup
                        .take()
                        .expect("the editor opens one window"),
                )
            })
    }
}
