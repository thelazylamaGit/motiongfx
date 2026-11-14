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

use motiongfx_2d::CompiledScene2dExt;
pub use vello_motiongfx::lettering as labels;
pub mod playback;
use playback::PlaybackControls;
pub use playback::{
    Playback, PlaybackConfig, PlaybackMode, PlaybackState,
};

/// Native preview window for an authored animation. Playback duration comes
/// from the compiled track, including waits; the window has no separate clock
/// range to keep in sync with the animation.
pub struct Window {
    title: &'static str,
    canvas: Size,
    playback_mode: PlaybackMode,
    renderer: vello_motiongfx::Renderer,
}

impl Window {
    pub fn new(
        title: &'static str,
        canvas: impl Into<(f64, f64)>,
    ) -> Self {
        let (width, height) = canvas.into();
        let canvas = Size::new(width, height);
        assert!(canvas.width.is_finite() && canvas.width > 0.0);
        assert!(canvas.height.is_finite() && canvas.height > 0.0);
        Self {
            title,
            canvas,
            playback_mode: PlaybackMode::Once,
            renderer: vello_motiongfx::Renderer::default(),
        }
    }

    pub fn playback_mode(mut self, mode: PlaybackMode) -> Self {
        self.playback_mode = mode;
        self
    }

    /// Use a configured scene renderer, for example with additional font files
    /// registered through `Renderer::fonts_mut()` before opening the window.
    pub fn renderer(
        mut self,
        renderer: vello_motiongfx::Renderer,
    ) -> Self {
        self.renderer = renderer;
        self
    }

    pub fn run(
        self,
        animation: motiongfx_2d::CompiledScene,
    ) -> Result<(), winit::error::EventLoopError> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        let mut app = VelloWinitApp::new(AnimationDemo {
            animation,
            renderer: self.renderer,
            title: self.title,
            canvas: self.canvas,
            viewport: self.canvas,
            playback_mode: self.playback_mode,
        });
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
        self.animation.background()
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

pub trait VelloDemo {
    fn window_title(&self) -> &'static str;
    /// Initial drawable size; the host adds space for playback controls.
    fn initial_logical_size(&self) -> (f64, f64);
    /// Clear color, read after rebuild_scene so animated backgrounds use the
    /// current sample. Preview controls paint their own background over it.
    fn background(&self) -> Color {
        Color::from_rgb8(28, 28, 46)
    }
    /// Drawable area above the host's playback controls, in logical pixels.
    fn size_changed(&mut self, size: Size);
    /// The preview range is independent of MotionGfx: a procedural demo
    /// can simply choose a duration and draw from `playback.time`.
    fn playback_config(&self) -> PlaybackConfig {
        PlaybackConfig::new(Duration::from_secs(30))
    }
    /// Draw at the supplied absolute time. The same callback is used for
    /// ordinary playback and seeking, with no timeline required.
    fn rebuild_scene(
        &mut self,
        scene: &mut Scene,
        scale_factor: f64,
        playback: PlaybackState,
    );

    /// Optional input handling. Cursor positions in `event` are physical
    /// pixels; divide by `scale_factor` for logical window coordinates.
    fn window_event(
        &mut self,
        _event: &WindowEvent,
        _scale_factor: f64,
    ) {
    }
}

pub struct VelloWinitApp<'s, D: VelloDemo> {
    pub context: RenderContext,
    pub renderer: Option<Renderer>,
    pub state: RenderState<'s>,
    pub scene: Scene,
    pub demo: D,
    controls: PlaybackControls,
    last_frame: Instant,
}

pub enum RenderState<'s> {
    Suspended(Option<Arc<WinitWindow>>),
    Active {
        surface: Box<RenderSurface<'s>>,
        window: Arc<WinitWindow>,
    },
}

impl<'s, D: VelloDemo> VelloWinitApp<'s, D> {
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
        }
    }

    fn tick_playback(&mut self) {
        let now = Instant::now();
        self.controls
            .clock
            .advance(now.duration_since(self.last_frame));
        self.last_frame = now;
    }

    fn render(&mut self) {
        self.tick_playback();
        let (surface, window) = match &mut self.state {
            RenderState::Active { surface, window } => {
                (surface, window)
            }
            _ => return,
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
            .unwrap()
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
            .unwrap();

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
            Arc::new(event_loop.create_window(attr).unwrap())
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
        let surface =
            pollster::block_on(surface_future).expect("surface");

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
                .unwrap(),
            );
        }

        self.state = RenderState::Active {
            surface: Box::new(surface),
            window,
        };
        self.last_frame = Instant::now();
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
        self.tick_playback();
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
                self.demo.window_event(&event, scale);
            }
        }
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                let scale_factor = match &self.state {
                    RenderState::Active { window, .. } => {
                        window.scale_factor()
                    }
                    _ => return,
                };
                self.handle_resize(scale_factor, size);
                self.render();
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
            }
            WindowEvent::RedrawRequested => {
                self.render();
                if let RenderState::Active { window, .. } =
                    &self.state
                {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
