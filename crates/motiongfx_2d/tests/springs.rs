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
                    sequence.play(root.spring_to(12.0, settings));
                    sequence.play(
                        b.position.spring_to((100.0, 50.0), settings),
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
                dot.position
                    .x
                    .spring_to(100.0, Spring::physical().mass(-1.0)),
            );
        }),
        Err(SceneError::Preparation(_))
    ));
}
