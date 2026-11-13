//! Real-font text with ordinary transforms and independently animated words.
//! Selections refer to source text, not glyph indices; shaping stays cached
//! while fill, opacity, position and scale animate. Scrub backwards to test it.

use motiongfx::prelude::{ms, s};
use motiongfx_2d::{FontWeight, Scene, Text, TextAlign, colors::*};
use vello_winit_motiongfx::Window;

fn main() -> Result<(), winit::error::EventLoopError> {
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
    let animation = scene.animate(|sequence| {
        let words = title.select("words");
        let move_word = title.select("move");
        sequence.parallel(|sequence| {
            sequence.play(title.opacity().to(1.0).over(ms(600)));
            sequence.play(title.position_y().to(30.0).over(ms(600)));
        });
        sequence.play(words.fill().to(ORANGE).over(ms(500)));
        sequence.play(move_word.opacity().to(0.15).over(ms(400)));
        sequence.play(move_word.opacity().to(1.0).over(ms(400)));
        sequence.play(title.scale().to(1.15).over(ms(600)));
        sequence.wait(s(1));
        sequence.play(title.opacity().to(0.0).over(ms(600)));
    });

    Window::new(
        "MotionGfx | Font text and word highlights",
        (1100.0, 500.0),
    )
    .run(animation)
}
