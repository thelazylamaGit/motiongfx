//! 2D objects follow simulations, directly and through `.map`.
use motiongfx::prelude::{ms, s};
use motiongfx_2d::{Circle, Point, Scene, wait};

#[test]
fn objects_follow_a_simulation_forwards_and_backwards() {
    let mut scene = Scene::new();
    scene.simulation_rate(10);
    // Grows by 1 every step: 10 per second.
    let size = scene.simulate(1.0, |size, _| *size += 1.0);
    let dot = scene.add(
        Circle::new(size)
            .position(size.map(|size| Point::new(size, 0.0))),
    );
    let mut animation = scene.compile([wait(s(1))]);

    for (time, expected) in
        [(ms(0), 1.0), (ms(500), 6.0), (s(1), 11.0), (ms(200), 3.0)]
    {
        animation.sample(time);
        let circle = animation.get(dot);
        assert_eq!(circle.radius, expected, "radius at {time:?}");
        assert_eq!(
            circle.transform.position,
            Point::new(expected, 0.0),
            "position at {time:?}"
        );
    }
}
