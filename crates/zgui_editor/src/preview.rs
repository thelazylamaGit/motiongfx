//! The scene preview: a zgui surface that Vello renders the sampled
//! animation into, on zgui's own device and texture.

use std::cell::Cell;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::time::Duration;

use vello::kurbo::Size;
use vello::{AaConfig, RenderParams, RendererOptions, Scene};
use zgui::surface::{SurfaceRenderCx, SurfaceRenderer, wgpu};

/// Draws the animation at the time the editor asks for.
pub(crate) struct Preview {
    animation: motiongfx_2d::CompiledScene,
    renderer: vello_motiongfx::Renderer,
    canvas: Size,
    /// The time to show, written by the editor's clock and controls.
    wanted: Rc<Cell<Duration>>,
    /// The time last rendered, or `None` before the first render.
    shown: Option<Duration>,
    scene: Scene,
    /// Vello's renderer and the device it was made on. zgui replaces the
    /// device after a loss, and the renderer has to follow it.
    gpu: Option<(wgpu::Device, vello::Renderer)>,
}

impl Preview {
    pub(crate) fn new(
        animation: motiongfx_2d::CompiledScene,
        renderer: vello_motiongfx::Renderer,
        canvas: Size,
        wanted: Rc<Cell<Duration>>,
    ) -> Self {
        Self {
            animation,
            renderer,
            canvas,
            wanted,
            shown: None,
            scene: Scene::new(),
            gpu: None,
        }
    }
}

impl SurfaceRenderer for Preview {
    fn render(&mut self, cx: &mut SurfaceRenderCx<'_>) {
        let time = self.wanted.get();
        self.shown = Some(time);

        if self
            .gpu
            .as_ref()
            .is_none_or(|(device, _)| device != cx.device)
        {
            let renderer = vello::Renderer::new(
                cx.device,
                RendererOptions {
                    use_cpu: false,
                    antialiasing_support: vello::AaSupport::area_only(
                    ),
                    num_init_threads: NonZeroUsize::new(1),
                    pipeline_cache: None,
                },
            )
            .unwrap_or_else(|error| {
                panic!("could not create the Vello renderer: {error}")
            });
            self.gpu = Some((cx.device.clone(), renderer));
        }
        let (_, vello) = self.gpu.as_mut().expect("created above");

        let scale = f64::from(cx.scale);
        let viewport = Size::new(
            f64::from(cx.size.width) / scale,
            f64::from(cx.size.height) / scale,
        );
        self.scene.reset();
        self.animation.sample(time);
        self.renderer.render(
            &self.animation,
            &mut self.scene,
            viewport,
            self.canvas,
            scale,
        );
        vello
            .render_to_texture(
                cx.device,
                cx.queue,
                &self.scene,
                cx.view,
                &RenderParams {
                    base_color: *self.animation.background(),
                    width: cx.size.width,
                    height: cx.size.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .unwrap_or_else(|error| {
                panic!("rendering failed: {error}")
            });
    }

    fn needs_render(&mut self) -> bool {
        self.shown != Some(self.wanted.get())
    }

    fn extra_usage(&self) -> wgpu::TextureUsages {
        // Vello's compute pass writes the texture directly.
        wgpu::TextureUsages::STORAGE_BINDING
    }
}
