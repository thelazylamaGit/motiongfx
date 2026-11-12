//! Headless conversion regressions kept outside the animation description.
use super::*;
use motiongfx::prelude::{TrackFragment, TrackOrdering};
use vello_motiongfx::Size;

// Original two-track ordering, using silent fragments to measure its schedule.
// The converted example has one timeline but must keep the same chapter spans.
fn original_timing() -> (Duration, Duration) {
    let grid = (0..(N_X + 2).max(N_Y + 2))
        .map(|index| {
            let column = (index < N_X + 2)
                .then(|| TrackFragment::silent(cs(60)));
            let row = (index < N_Y + 2)
                .then(|| TrackFragment::silent(cs(60)));
            column.into_iter().chain(row).ord_flow(ms(25))
        })
        .ord_flow(cs(5))
        .compile()
        .duration();
    let curves = (2..=N_X + N_Y)
        .map(|diagonal| {
            (1..=N_X)
                .filter_map(|column| {
                    let row = diagonal.checked_sub(column)?;
                    (1..=N_Y).contains(&row).then(|| {
                        [
                            TrackFragment::silent(DRAW_DUR),
                            TrackFragment::silent(DRAW_DUR),
                        ]
                        .ord_flow(DRAW_DUR + HOLD_DUR)
                    })
                })
                .ord_all()
        })
        .ord_flow(STAGGER)
        .compile()
        .duration();
    (grid, grid + curves)
}

fn frame(animation: &mut Animation, time: Duration) -> vello::Scene {
    animation.sample(time);
    let mut frame = vello::Scene::new();
    let canvas = Size::new(CANVAS.0, CANVAS.1);
    vello_motiongfx::Renderer::default()
        .render(&animation, &mut frame, canvas, canvas, 1.0);
    frame
}

#[test]
fn chapter_timing_matches_the_original_and_seek_order_does_not_change_the_picture()
 {
    let (mut animation, loop_start) = build_animation();
    let (original_intro, original_end) = original_timing();
    assert_eq!(loop_start, original_intro);
    assert_eq!(animation.duration(), original_end);

    let initial = frame(&mut animation, Duration::ZERO);
    let introduced = frame(&mut animation, loop_start);
    // Four primitives per reference circle; grid lines appear during intro.
    assert_eq!(initial.encoding().n_paths as usize, (N_X + N_Y) * 4);
    assert_eq!(
        introduced.encoding().n_paths as usize,
        (N_X + N_Y) * 4 + (N_X + 2) + (N_Y + 2)
    );

    // Exercise backward seeks across the chapter boundary and a loop restart.
    for time in [
        original_end,
        loop_start + ms(500),
        Duration::ZERO,
        loop_start - ms(25),
        loop_start,
        loop_start + ms(1234),
        original_end,
        loop_start,
    ] {
        let actual = frame(&mut animation, time);
        let (mut fresh, _) = build_animation();
        let expected = frame(&mut fresh, time);
        assert_eq!(
            actual.encoding().path_data,
            expected.encoding().path_data
        );
        assert_eq!(
            actual.encoding().draw_data,
            expected.encoding().draw_data
        );
        assert_eq!(
            actual.encoding().transforms,
            expected.encoding().transforms
        );
        assert_eq!(
            actual.encoding().styles,
            expected.encoding().styles
        );
    }
}
