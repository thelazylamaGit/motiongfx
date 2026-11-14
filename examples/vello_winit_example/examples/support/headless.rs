//! Shared headless check for examples: the animation compiles (building it
//! panics otherwise), samples at any time including backwards, and renders
//! a frame, without opening a window.
use motiongfx_2d::CompiledScene;
use std::time::Duration;
use vello_motiongfx::{Renderer, Size};

pub fn check(mut animation: CompiledScene) {
    let end = animation.duration();
    let size = Size::new(1280.0, 720.0);
    let mut renderer = Renderer::default();
    let mut scene = vello::Scene::new();
    for time in
        [Duration::ZERO, end / 2, end, end / 3, Duration::ZERO]
    {
        animation.sample(time);
        scene.reset();
        renderer.render(&animation, &mut scene, size, size, 1.0);
    }
}
