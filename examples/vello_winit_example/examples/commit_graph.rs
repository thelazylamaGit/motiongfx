//! Canvas Commons commit graph, using ordinary circles, rings and paths.
//! A reusable component creates its own objects; choreography stays below.

use motiongfx::prelude::{ease, ms, s};
use motiongfx_2d::{
    Animation, Circle, CircleHandle, Color, Path, Scene, all,
    colors::{MAUVE, PASTEL_BLUE, TRANSPARENT},
    flow, wait,
};
use vello_winit_motiongfx::Window;

struct Commit {
    dot: CircleHandle,
    ring: CircleHandle,
}

impl Commit {
    fn new(
        scene: &mut Scene,
        position: (f64, f64),
        fill: Color,
    ) -> Self {
        let dot = scene.add(
            Circle::new(18.0)
                .position(position)
                .fill(fill)
                .scale(0.0),
        );
        let ring = scene.add(
            Circle::new(18.0)
                .relative_to(dot)
                .fill(TRANSPARENT)
                .stroke(fill, 3.0),
        );
        Self { dot, ring }
    }

    /// Grow the dot in; its child ring follows the dot's transform.
    fn appear(&self) -> Animation {
        self.dot
            .scale()
            .to(1.0)
            .over(ms(400))
            .ease(ease::back::ease_out)
    }

    /// Same pulse as scale = 1 + pulse*1.4 and opacity = 1 - pulse,
    /// expressed directly as two ordinary ring property animations.
    fn pulse(&self) -> Animation {
        all([self.ring.scale().to(2.4), self.ring.opacity().to(0.0)])
    }
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let mut scene = Scene::new();
    // Positive y points up here, so negate Canvas Commons' y coordinates.
    let trunk = scene.add(
        Path::new([(-360.0, -60.0), (200.0, -60.0)])
            .stroke(PASTEL_BLUE, 4.0)
            .end(0.0),
    );
    let fork = scene.add(
        Path::new([(-120.0, -60.0), (40.0, 60.0), (240.0, 60.0)])
            .stroke(MAUVE, 4.0)
            .end(0.0),
    );
    let commits = [
        Commit::new(&mut scene, (-360.0, -60.0), PASTEL_BLUE),
        Commit::new(&mut scene, (-120.0, -60.0), PASTEL_BLUE),
        Commit::new(&mut scene, (200.0, -60.0), PASTEL_BLUE),
        Commit::new(&mut scene, (40.0, 60.0), MAUVE),
        Commit::new(&mut scene, (240.0, 60.0), MAUVE),
    ];

    scene.defaults(s(1), ease::linear);
    let animation = scene.compile([
        trunk.end().to(1.0).over(ms(800)),
        flow(ms(120), commits.iter().map(Commit::appear)),
        fork.end().to(1.0).over(ms(800)),
        all(commits.iter().map(Commit::pulse)),
        wait(ms(400)),
    ]);

    Window::new("MotionGfx | Commit graph", (960.0, 540.0))
        .run(animation)
}
