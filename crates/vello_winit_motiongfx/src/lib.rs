//! A native preview window for MotionGfx animations: renders a compiled
//! scene with Vello and adds play/pause and scrubbing controls.
//!
//! ```no_run
//! use std::time::Duration;
//! use motiongfx_2d::prelude::*;
//! use vello_winit_motiongfx::Window;
//!
//! let mut scene = Scene::new();
//! let dot = scene.add(Circle::new(20.0));
//! let animation = scene.compile([
//!     dot.position_x().to(200.0).over(Duration::from_millis(800)),
//! ]);
//! Window::new("Moving dot", (800.0, 450.0)).run(animation)?;
//! # Ok::<(), winit::error::EventLoopError>(())
//! ```
//!
//! For custom drawing without a compiled scene, implement [`VelloDemo`]
//! and run it with [`VelloWinitApp`].
#![warn(missing_docs)]
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use vello::kurbo::Size;
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{
    AaConfig, RenderParams, Renderer, RendererOptions, Scene,
};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window as WinitWindow;

pub use vello_motiongfx::lettering as labels;
#[cfg(feature = "audio")]
pub mod audio;
pub mod playback;
use playback::PlaybackControls;
pub use playback::{
    Playback, PlaybackConfig, PlaybackMode, PlaybackState,
};

/// A preview window for a compiled scene. Playback runs for the
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
    /// on the origin. The canvas is scaled to fit the window.
    ///
    /// # Panics
    ///
    /// If the width or height isn't finite and positive.
    pub fn new(
        title: &'static str,
        canvas: impl Into<(f64, f64)>,
    ) -> Self {
        let (width, height) = canvas.into();
        let canvas = Size::new(width, height);
        assert!(
            canvas.width.is_finite() && canvas.width > 0.0,
            "canvas width must be finite and positive, got {width}"
        );
        assert!(
            canvas.height.is_finite() && canvas.height > 0.0,
            "canvas height must be finite and positive, got {height}"
        );
        Self {
            title,
            canvas,
            playback_mode: PlaybackMode::Once,
            renderer: vello_motiongfx::Renderer::default(),
            #[cfg(feature = "audio")]
            audio: false,
        }
    }

    /// Play the scene's sound events (notes and samples from
    /// `motiongfx_audio`) through the default output device. Sound plays
    /// while time moves forwards and stops on a pause, seek or reverse;
    /// resuming starts with the events after the new time. If the device
    /// can't be opened, the preview runs silently and says why on stderr.
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
    /// If the windowing event loop can't be created or fails.
    ///
    /// # Panics
    ///
    /// If the window, GPU surface or Vello renderer can't be created.
    pub fn run(
        self,
        animation: motiongfx_2d::CompiledScene,
    ) -> Result<(), winit::error::EventLoopError> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        #[cfg(feature = "audio")]
        let audio = self
            .audio
            .then(|| {
                audio::AudioSync::open(&animation, self.playback_mode)
            })
            .flatten();
        let mut app = VelloWinitApp::new(AnimationDemo {
            animation,
            renderer: self.renderer,
            title: self.title,
            canvas: self.canvas,
            viewport: self.canvas,
            playback_mode: self.playback_mode,
        });
        #[cfg(feature = "audio")]
        {
            app.audio = audio;
        }
        event_loop.run_app(&mut app)
    }
}

struct AnimationDemo {
    renderer: vello_motiongfx::Renderer,
    animation: motiongfx_2d::CompiledScene,
    title: &'static str,
    canvas: Size,
    viewport: Size,
    playback_mode: PlaybackMode,
}

impl VelloDemo for AnimationDemo {
    fn window_title(&self) -> &'static str {
        self.title
    }
    fn initial_logical_size(&self) -> (f64, f64) {
        (self.canvas.width, self.canvas.height)
    }
    fn size_changed(&mut self, size: Size) {
        self.viewport = size;
    }
    fn playback_config(&self) -> PlaybackConfig {
        PlaybackConfig::new(self.animation.duration())
            .with_mode(self.playback_mode)
    }
    fn background(&self) -> Color {
        *self.animation.background()
    }
    fn rebuild_scene(
        &mut self,
        scene: &mut Scene,
        dpi: f64,
        playback: PlaybackState,
    ) {
        self.animation.sample(playback.time);
        self.renderer.render(
            &self.animation,
            scene,
            self.viewport,
            self.canvas,
            dpi,
        );
    }
}

/// Custom content for the preview window, drawn directly with Vello from
/// the playback state. [`Window`] uses this to show a compiled scene;
/// implement it for drawing that doesn't use one.
pub trait VelloDemo {
    /// The window's title.
    fn window_title(&self) -> &'static str;
    /// The drawing area's starting size in logical pixels; the window
    /// adds room for the controls below it.
    fn initial_logical_size(&self) -> (f64, f64);
    /// The colour the frame is cleared to. Read after `rebuild_scene`, so
    /// it can change with the animation.
    fn background(&self) -> Color {
        Color::from_rgb8(28, 28, 46)
    }
    /// The drawing area (above the controls) changed to `size`, in logical
    /// pixels.
    fn size_changed(&mut self, size: Size);
    /// How long playback runs and what happens at the end. Defaults to
    /// 30 seconds, played once.
    fn playback_config(&self) -> PlaybackConfig {
        PlaybackConfig::new(Duration::from_secs(30))
    }
    /// Draw the frame for `playback.time` into `scene` (already cleared).
    /// Called for playback and seeking alike, so draw from the time rather
    /// than from the previous frame.
    fn rebuild_scene(
        &mut self,
        scene: &mut Scene,
        scale_factor: f64,
        playback: PlaybackState,
    );

    /// Handle a window event the controls didn't use. Cursor positions are
    /// in physical pixels; divide by `scale_factor` for logical ones.
    /// Return `true` if the event changed what the demo draws, so the
    /// window redraws; it otherwise redraws only for playback and window
    /// changes.
    fn window_event(
        &mut self,
        _event: &WindowEvent,
        _scale_factor: f64,
    ) -> bool {
        false
    }
}

/// The preview application: runs a [`VelloDemo`] in a window with
/// playback controls. Pass it to winit's `EventLoop::run_app`.
pub struct VelloWinitApp<'s, D: VelloDemo> {
    /// Vello's GPU devices and surfaces.
    pub context: RenderContext,
    /// Vello's renderer, created when the window first appears.
    pub renderer: Option<Renderer>,
    /// The window and its surface, while it exists.
    pub state: RenderState<'s>,
    /// The frame being drawn, rebuilt each frame.
    pub scene: Scene,
    /// The content being shown.
    pub demo: D,
    controls: PlaybackControls,
    last_frame: Instant,
    /// Hidden behind other windows. Not every platform reports this.
    occluded: bool,
    /// The scene's sound, driven by the clock; see [`Window::audio`].
    #[cfg(feature = "audio")]
    audio: Option<audio::AudioSync>,
}

/// Whether the app has a window to draw to.
pub enum RenderState<'s> {
    /// No surface; holds the window to reuse on resume, if one was made.
    Suspended(Option<Arc<WinitWindow>>),
    /// Drawing to `window` through `surface`.
    Active {
        /// The GPU surface.
        surface: Box<RenderSurface<'s>>,
        /// The window.
        window: Arc<WinitWindow>,
    },
}

impl<'s, D: VelloDemo> VelloWinitApp<'s, D> {
    /// An app showing `demo`; the window opens when the event loop starts.
    pub fn new(demo: D) -> Self {
        let controls = PlaybackControls::new(demo.playback_config());
        Self {
            context: RenderContext::new(),
            renderer: None,
            state: RenderState::Suspended(None),
            scene: Scene::new(),
            demo,
            controls,
            last_frame: Instant::now(),
            occluded: false,
            #[cfg(feature = "audio")]
            audio: None,
        }
    }

    fn request_redraw(&self) {
        if let RenderState::Active { window, .. } = &self.state {
            window.request_redraw();
        }
    }

    /// Whether to keep drawing frames: only while time moves and the
    /// window is visible. Otherwise the host waits for a change.
    fn animating(&self) -> bool {
        let RenderState::Active { window, .. } = &self.state else {
            return false;
        };
        let size = window.inner_size();
        self.controls.clock.state().playing
            && !self.occluded
            && size.width > 0
            && size.height > 0
    }

    fn tick_playback(&mut self) {
        let now = Instant::now();
        #[cfg(feature = "audio")]
        let before = self.controls.clock.state();
        self.controls
            .clock
            .advance(now.duration_since(self.last_frame));
        self.last_frame = now;
        #[cfg(feature = "audio")]
        if let Some(audio) = &mut self.audio {
            audio.update(before, self.controls.clock.state());
        }
    }

    fn render(&mut self) {
        self.tick_playback();
        let RenderState::Active { surface, window } = &mut self.state
        else {
            return;
        };

        self.scene.reset();

        let size = window.inner_size();
        let scale_factor = window.scale_factor();

        if size.width == 0 || size.height == 0 {
            return;
        }

        if surface.config.width != size.width
            || surface.config.height != size.height
        {
            self.context.resize_surface(
                surface,
                size.width,
                size.height,
            );
        }

        self.demo.rebuild_scene(
            &mut self.scene,
            scale_factor,
            self.controls.clock.state(),
        );
        self.controls.draw(
            &mut self.scene,
            scale_factor,
            Size::new(
                size.width as f64 / scale_factor,
                size.height as f64 / scale_factor,
            ),
        );

        let dev = &self.context.devices[surface.dev_id];
        let texture = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => {
                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(
                surface_texture,
            ) => {
                self.context.resize_surface(
                    surface,
                    size.width,
                    size.height,
                );
                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost => {
                self.context.resize_surface(
                    surface,
                    size.width,
                    size.height,
                );
                return;
            }
            _ => return,
        };

        self.renderer
            .as_mut()
            .expect("the renderer is created when the window resumes")
            .render_to_texture(
                &dev.device,
                &dev.queue,
                &self.scene,
                &surface.target_view,
                &RenderParams {
                    base_color: self.demo.background(),
                    width: surface.config.width,
                    height: surface.config.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .unwrap_or_else(|error| {
                panic!("rendering failed: {error}")
            });

        let mut enc = dev.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: None },
        );

        surface.blitter.copy(
            &dev.device,
            &mut enc,
            &surface.target_view,
            &texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );

        dev.queue.submit([enc.finish()]);
        dev.queue.present(texture);
    }

    fn handle_resize(
        &mut self,
        scale_factor: f64,
        size: PhysicalSize<u32>,
    ) {
        let logical_width = size.width as f64 / scale_factor;
        let logical_height = size.height as f64 / scale_factor;
        self.demo.size_changed(Size::new(
            logical_width,
            (logical_height - PlaybackControls::HEIGHT).max(0.0),
        ));
        self.controls.resized();
        self.last_frame = Instant::now();
    }
}

impl<D: VelloDemo> ApplicationHandler for VelloWinitApp<'_, D> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state
        else {
            return;
        };

        let window = cached_window.take().unwrap_or_else(|| {
            let (w, h) = self.demo.initial_logical_size();
            let attr = WinitWindow::default_attributes()
                .with_inner_size(LogicalSize::new(
                    w,
                    h + PlaybackControls::HEIGHT,
                ))
                .with_title(self.demo.window_title());
            Arc::new(event_loop.create_window(attr).unwrap_or_else(
                |error| {
                    panic!("could not create the window: {error}")
                },
            ))
        });

        self.handle_resize(
            window.scale_factor(),
            window.inner_size(),
        );

        let size = window.inner_size();
        let surface_future = self.context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        );
        let surface = pollster::block_on(surface_future)
            .unwrap_or_else(|error| {
                panic!("could not create a GPU surface: {error}")
            });

        let device_handle = &self.context.devices[surface.dev_id];
        surface
            .surface
            .configure(&device_handle.device, &surface.config);

        if self.renderer.is_none() {
            self.renderer = Some(
                Renderer::new(
                    &device_handle.device,
                    RendererOptions {
                        use_cpu: false,
                        antialiasing_support:
                            vello::AaSupport::area_only(),
                        num_init_threads: NonZeroUsize::new(1),
                        pipeline_cache: None,
                    },
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "could not create the Vello renderer: {error}"
                    )
                }),
            );
        }

        self.state = RenderState::Active {
            surface: Box::new(surface),
            window,
        };
        self.last_frame = Instant::now();
        self.request_redraw();
    }

    fn suspended(&mut self, _el: &ActiveEventLoop) {
        self.controls.clock.pause();
        self.controls.resized();
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window_event(
        &mut self,
        el: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::RedrawRequested = event {
            self.render();
            // Keep the frame loop going only while playing; vsync paces it.
            if self.animating() {
                self.request_redraw();
            }
            return;
        }
        self.tick_playback();
        let before = self.controls.clock.state();
        let mut redraw = false;
        if let RenderState::Active { window, .. } = &self.state {
            let scale = window.scale_factor();
            let size = window.inner_size();
            let consumed = self.controls.handle_event(
                &event,
                scale,
                Size::new(
                    size.width as f64 / scale,
                    size.height as f64 / scale,
                ),
            );
            if !consumed {
                redraw = self.demo.window_event(&event, scale);
            }
        }
        // A seek, play/pause or loss of focus changes the picture.
        redraw |= self.controls.clock.state() != before;
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                let RenderState::Active { window, .. } = &self.state
                else {
                    return;
                };
                let scale_factor = window.scale_factor();
                self.handle_resize(scale_factor, size);
                // Draw now for smooth resizing; the request also restarts
                // the frame loop after the window is restored.
                self.render();
                redraw = true;
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor, ..
            } => {
                if let RenderState::Active { window, .. } =
                    &self.state
                {
                    self.handle_resize(
                        scale_factor,
                        window.inner_size(),
                    );
                }
                redraw = true;
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                redraw |= !occluded;
            }
            _ => {}
        }
        if redraw {
            self.request_redraw();
        }
    }
}
