use motiongfx::prelude::{ms, s};
use motiongfx_2d::{
    Animation2dExt, Circle, Point, Scene, SceneError, Spring, signal,
};

#[test]
fn properties_positions_and_signals_use_core_spring_sampling() {
    let cases: [Spring; 2] = [
        Spring::visual(ms(500)).bounce(0.5).into(),
        Spring::timed(ms(500)).bounce(0.5).into(),
    ];
    for settings in cases {
        let curve = settings.prepare().unwrap();
        let root = signal(2.0);
        let mut scene = Scene::new();
        let a = scene.add(Circle::new(&root));
        let b = scene.add(Circle::new(3.0));
        let mut animation = scene
            .try_animate(|sequence| {
                sequence.parallel(|sequence| {
                    sequence.play(root.to(12.0).spring(settings));
                    sequence.play(
                        b.position()
                            .to_xy(100.0, 50.0)
                            .spring(settings),
                    );
                });
                sequence.wait(s(1));
            })
            .unwrap();
        assert_eq!(animation.duration(), curve.duration() + s(1));
        for time in [ms(250), curve.duration(), ms(100), ms(0)] {
            animation.sample(time);
            let amount = curve.sample(time).position;
            assert!(
                (animation.circle(a).radius - (2.0 + 10.0 * amount))
                    .abs()
                    < 0.0001
            );
            let expected = Point::new(amount * 100.0, amount * 50.0);
            assert!(
                animation.circle(b).position.distance(expected)
                    < 0.0001
            );
        }
    }
}

#[test]
fn checked_preparation_reports_invalid_spring_options() {
    let mut scene = Scene::new();
    let dot = scene.add(Circle::new(2.0));
    assert!(matches!(
        scene.try_animate(|sequence| {
            sequence.play(
                dot.position_x()
                    .to(100.0)
                    .spring(Spring::physical().mass(-1.0)),
            );
        }),
        Err(SceneError::Preparation(_))
    ));
}

#[test]
fn relative_helpers_use_spring_timing_and_preserve_action_start_values()
 {
    let settings: Spring = Spring::timed(ms(500)).bounce(0.4).into();
    let curve = settings.prepare().unwrap();
    let mut scene = Scene::new();
    let dot = scene.add(
        Circle::new(2.0)
            .position((20.0, -10.0))
            .scale(2.0)
            .rotation(0.2),
    );
    let mut animation = scene.animate(|sequence| {
        sequence.parallel(|sequence| {
            sequence.play(dot.shift(100.0, 50.0).spring(settings));
            sequence.play(dot.scale_by(0.5).spring(settings));
            sequence.play(dot.rotate(0.4).spring(settings));
        });
    });
    assert_eq!(animation.duration(), curve.duration());
    for time in [ms(0), ms(250), ms(500), ms(100), ms(0)] {
        animation.sample(time);
        let amount = curve.sample(time).position;
        let state = animation.circle(dot);
        assert!(
            state.position.distance(Point::new(
                20.0 + 100.0 * amount,
                -10.0 + 50.0 * amount
            )) < 1e-4
        );
        assert!((state.scale - (2.0 - amount)).abs() < 1e-4);
        assert!((state.rotation - (0.2 + 0.4 * amount)).abs() < 1e-4);
    }
}

#[test]
fn explicit_tween_timing_cannot_silently_override_a_spring() {
    use motiongfx_authoring::PreparationError;
    for duration_option in [true, false] {
        let mut scene = Scene::new();
        let dot = scene.add(Circle::new(2.0));
        let result = scene.try_animate(|sequence| {
            let change = dot.radius().to(20.0);
            let change = if duration_option {
                change.over(ms(200))
            } else {
                change.ease(motiongfx::prelude::ease::linear)
            };
            sequence.play(change.spring(Spring::physical()));
        });
        assert!(matches!(
            result,
            Err(SceneError::Preparation(
                PreparationError::ConflictingSpringTiming
            ))
        ));
    }
}
