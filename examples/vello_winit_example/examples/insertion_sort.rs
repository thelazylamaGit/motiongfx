//! Insertion sort: Rust generates the timeline once; playback only samples it.
//! Labels use positions local to their bars, so they need no movement tracks.

use motiongfx::prelude::{ease, ms};
use motiongfx_2d::{
    Rectangle, Scene, Text, TextAlign, all, colors::PASTEL_BLUE, wait,
};
use vello_winit_motiongfx::Window;

const VALUES: &[u32] = &[8, 4, 1, 2, 5, 6, 3, 7];

fn slot(index: usize, count: usize) -> f64 {
    let middle = (count as f64 - 1.0) / 2.0;
    (index as f64 - middle) * 90.0
}

fn build_animation() -> motiongfx_2d::CompiledScene {
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

    scene.defaults(ms(400), ease::cubic::ease_in_out);
    let mut steps = Vec::new();
    for i in 1..order.len() {
        let mut j = i;
        while j > 0 && order[j - 1].0 > order[j].0 {
            order.swap(j - 1, j);
            j -= 1;
        }
        steps.push(status.set_content(format!(
            "INSERTION PASS {i} / {}",
            order.len() - 1
        )));
        // All bars move together; their labels follow automatically.
        steps.push(all(order.iter().enumerate().map(
            |(index, (_, bar))| {
                bar.position_x().to(slot(index, order.len()))
            },
        )));
    }
    steps.push(status.set_content("SORTED / HOLD"));
    steps.push(wait(ms(600)));
    scene.compile(steps)
}

fn main() -> Result<(), winit::error::EventLoopError> {
    let animation = build_animation();

    Window::new("MotionGfx | Insertion sort", (900.0, 600.0))
        .run(animation)
}

#[cfg(test)]
#[path = "support/headless.rs"]
mod headless;

#[test]
fn builds_and_samples_headlessly() {
    headless::check(build_animation());
}
