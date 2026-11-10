//! Insertion sort: Rust generates the timeline once; playback only samples it.
//! Labels use positions local to their bars, so they need no movement tracks.

use motiongfx::prelude::{ease, ms};
use vello_motiongfx::{
    Rectangle, Scene, Text, TextAlign, colors::PASTEL_BLUE,
};
use vello_winit_motiongfx::Window;

const VALUES: &[u32] = &[5, 2, 8, 1, 6, 3];

fn slot(index: usize, count: usize) -> f64 {
    let middle = (count as f64 - 1.0) / 2.0;
    (index as f64 - middle) * 90.0
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let mut scene = Scene::new();

    scene.add(
        Text::new("Insertion sort")
            .position((-400.0, 260.0))
            .font_size(21.0)
            .fill(PASTEL_BLUE),
    );

    // Format the current input, including any changes to VALUES above.
    let input = VALUES
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" ");

    scene.add(
        Text::new(format!("INPUT  {input}"))
            .position((-400.0, 224.0))
            .font_size(14.0),
    );

    let status = scene.add(
        Text::new("SORTED / HOLD")
            .position((-400.0, 190.0))
            .font_size(10.5),
    );

    let mut order: Vec<_> = VALUES
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            let height = f64::from(value) * 36.0;
            let bar = scene.add(
                Rectangle::new(64.0, height)
                    .position((
                        slot(index, VALUES.len()),
                        height / 2.0 - 200.0,
                    ))
                    .radius(8.0)
                    .fill(PASTEL_BLUE),
            );
            scene.add(
                Text::new(value.to_string())
                    .relative_to(bar)
                    .position((0.0, -height / 2.0 - 12.0))
                    .font_size(14.0)
                    .align(TextAlign::Center),
            );
            (value, bar)
        })
        .collect();

    let animation = scene.animate(|sequence| {
        sequence.defaults(ms(400), ease::cubic::ease_in_out);
        for i in 1..order.len() {
            let mut j = i;
            while j > 0 && order[j - 1].0 > order[j].0 {
                order.swap(j - 1, j);
                j -= 1;
            }
            sequence.play(status.content.set(format!(
                "INSERTION PASS {i} / {}",
                order.len() - 1
            )));
            // All bars move together; their labels follow automatically.
            sequence.play_all(order.iter().enumerate().map(
                |(index, (_, bar))| {
                    bar.position.x.to(slot(index, order.len()))
                },
            ));
        }
        sequence.play(status.content.set("SORTED / HOLD"));
        sequence.wait(ms(600));
    });

    Window::new("MotionGfx | Insertion sort", (900.0, 600.0))
        .run(animation)
}
