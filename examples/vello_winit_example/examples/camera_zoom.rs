//! Manim's camera-frame zoom, using an ordinary MotionGfx timeline.
//! Geometry is created once. Only the shared view transform is animated.

use motiongfx::prelude::s;
use motiongfx_2d::{
    Circle, Path, Scene, Scene2dExt as _, all,
    colors::{BLUE, TRANSPARENT, YELLOW},
    wait,
};
use vello_winit_motiongfx::Window;

fn build_animation() -> motiongfx_2d::CompiledScene {
    let mut scene = Scene::new();

    // Map each Manim unit to 100 logical pixels. Positive y points up.
    scene.add(Circle::new(300.0).fill(TRANSPARENT).stroke(BLUE, 4.0));
    scene.add(
        Path::new([(-400.0, 300.0), (400.0, 300.0)])
            .stroke(YELLOW, 4.0),
    );

    let camera = scene.camera();
    // Animate the frame's size, not magnification: 1/4 size means 4x
    // magnification. Position and size use the same smooth progress.
    scene.compile([
        all([
            camera.scale().to(0.25).over(s(2)),
            camera.position().to_xy(0.0, 300.0).over(s(2)),
        ]),
        wait(s(1)),
        all([
            camera.scale().to(1.0).over(s(2)),
            camera.position().to_xy(0.0, 0.0).over(s(2)),
        ]),
    ])
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let animation = build_animation();

    // The 800px-high canvas leaves room to see the full 600px circle.
    // Shared playback controls stay outside the animated camera view.
    Window::new("MotionGfx | Camera zoom", (1200.0, 800.0))
        .run(animation)
}

#[cfg(test)]
#[path = "support/headless.rs"]
mod headless;

#[test]
fn builds_and_samples_headlessly() {
    headless::check(build_animation());
}
