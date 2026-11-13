//! Compare reused render/cache state with fresh arbitrary-time construction.
//! This exercises spring runs, settings morphs and backward seeks together.
use super::*;
use motiongfx_2d::Size;

#[test]
fn curve_geometry_and_spring_balls_are_independent_of_seek_history() {
    let mut animation = build_animation();
    let duration = animation.duration();
    let size = Size::new(CANVAS.0, CANVAS.1);
    let mut renderer = vello_motiongfx::Renderer::default();
    // Explicitly sample the next preset transition, including backwards through it.
    let first = &experiments()[0];
    let second_start = [first.physical, first.visual, first.timed]
        .into_iter()
        .map(|s| s.prepare().unwrap().duration())
        .max()
        .unwrap()
        + ms(1800);
    let morph_times = [ms(250), ms(750), ms(500)]
        .map(|offset| second_start + offset);
    let mut morph_paths = Vec::new();
    let rewind_times = [ms(400), ms(100), ms(250)]
        .map(|offset| second_start - offset);
    let mut rewind_transforms = Vec::new();
    for time in [
        ms(0),
        ms(750),
        rewind_times[0],
        rewind_times[1],
        rewind_times[2],
        second_start + ms(250),
        second_start + ms(750),
        second_start + ms(500),
        duration,
        duration.mul_f64(0.4),
        ms(750),
        ms(0),
        duration.mul_f64(0.8),
    ] {
        animation.sample(time);
        let mut actual = vello::Scene::new();
        renderer.render(&animation, &mut actual, size, size, 1.0);
        if morph_times.contains(&time) {
            morph_paths.push(actual.encoding().path_data.clone());
        }
        if rewind_times.contains(&time) {
            rewind_transforms
                .push(actual.encoding().transforms.clone());
        }
        let mut fresh = build_animation();
        fresh.sample(time);
        let mut expected = vello::Scene::new();
        vello_motiongfx::Renderer::default().render(
            &fresh,
            &mut expected,
            size,
            size,
            1.0,
        );
        assert!(actual.encoding().n_paths > 0);
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
    assert_eq!(morph_paths.len(), 3);
    assert!(morph_paths.windows(2).all(|pair| pair[0] != pair[1]));
    assert_eq!(rewind_transforms.len(), 3);
    assert!(
        rewind_transforms.windows(2).all(|pair| pair[0] != pair[1])
    );
}
