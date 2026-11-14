//! Real-font text with ordinary transforms and independently animated words.
//! Selections refer to source text, not glyph indices; shaping stays cached
//! while fill, opacity, position and scale animate. Scrub backwards to test it.

use motiongfx::prelude::{ms, s};
use motiongfx_2d::{
    FontWeight, Scene, Text, TextAlign, all, colors::*, wait,
};
use vello_winit_motiongfx::Window;

fn build_animation() -> motiongfx_2d::CompiledScene {
    let mut scene = Scene::new();
    // Fira Sans is embedded by default. Register other font files on the
    // renderer before using their family names; no OS font install is needed.
    let title = scene.add(
        Text::new("Make words move.")
            .font_size(72.0)
            .font_weight(FontWeight::BOLD)
            .align(TextAlign::Center)
            .position((0.0, -30.0))
            .opacity(0.0),
    );
    let words = title.select("words");
    let move_word = title.select("move");
    scene.compile([
        all([
            title.opacity().to(1.0).over(ms(600)),
            title.position_y().to(30.0).over(ms(600)),
        ]),
        words.fill().to(ORANGE).over(ms(500)),
        move_word.opacity().to(0.15).over(ms(400)),
        move_word.opacity().to(1.0).over(ms(400)),
        title.scale().to(1.15).over(ms(600)),
        wait(s(1)),
        title.opacity().to(0.0).over(ms(600)),
    ])
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let animation = build_animation();

    Window::new(
        "MotionGfx | Font text and word highlights",
        (1100.0, 500.0),
    )
    .run(animation)
}

#[cfg(test)]
#[path = "support/headless.rs"]
mod headless;

#[test]
fn builds_and_samples_headlessly() {
    headless::check(build_animation());
}
