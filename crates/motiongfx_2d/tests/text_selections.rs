use motiongfx::prelude::{ease, ms, s};
use motiongfx_2d::{Scene, SceneError, Text, colors};

#[test]
fn occurrence_and_range_queries_reuse_registered_paint_state() {
    let mut scene = Scene::new();
    let title = scene.add(Text::new("Shoes & Cats & Dogs"));
    let first = title.select("&");
    let second = title.select_nth("&", 1);
    let same_second = title.select_range(13..14);
    let mut animation = scene.animate(|sequence| {
        sequence.defaults(s(1), ease::linear);
        sequence.play(first.fill().to(colors::BLUE));
        sequence.play(second.opacity().to(0.0));
        sequence.play(same_second.opacity().to(1.0));
        // A separately constructed equivalent query also finds the same state.
        sequence
            .play(title.select_nth("&", 1).fill().to(colors::RED));
    });
    assert_eq!(animation.model().text_selections().len(), 2);
    for (time, opacity) in
        [(2000, 0.0), (2500, 0.5), (3000, 1.0), (1500, 0.5)]
    {
        animation.sample(ms(time));
        let selections = animation.model().text_selections();
        assert_eq!(selections[0].range.range(), 6..7);
        assert_eq!(selections[1].range.range(), 13..14);
        assert_eq!(selections[1].opacity, opacity);
    }
}

#[test]
fn invalid_used_queries_return_helpful_preparation_errors() {
    for query in ["missing", ""] {
        let mut scene = Scene::new();
        let title = scene.add(Text::new("hello"));
        let result = scene.try_animate(|sequence| {
            sequence.play(title.select(query).fill().to(colors::RED));
        });
        match result {
            Err(error @ SceneError::Animation(_)) => {
                let message = error.to_string();
                assert!(message.contains(query));
                assert!(message.contains("hello"));
                assert!(message.contains("text_selections.rs"));
            }
            _ => panic!("expected selection preparation error"),
        }
    }
    let mut scene = Scene::new();
    let title = scene.add(Text::new("café"));
    assert!(matches!(
        scene.try_animate(|sequence| {
            sequence.play(title.select_range(3..4).opacity().to(0.0));
        }),
        Err(SceneError::Animation(_))
    ));
}

#[test]
fn deferred_selection_springs_use_core_sampling() {
    use motiongfx_2d::Spring;
    let settings: Spring = Spring::timed(ms(500)).bounce(0.3).into();
    let curve = settings.prepare().unwrap();
    let mut scene = Scene::new();
    let title = scene.add(Text::new("hello"));
    let mut animation = scene.animate(|sequence| {
        sequence.play(
            title.select("hello").opacity().to(0.0).spring(settings),
        );
    });
    for time in [ms(250), ms(500), ms(0)] {
        animation.sample(time);
        let opacity = animation.model().text_selections()[0].opacity;
        assert!(
            (opacity - (1.0 - curve.sample(time).position)).abs()
                < 1e-6
        );
    }
}
