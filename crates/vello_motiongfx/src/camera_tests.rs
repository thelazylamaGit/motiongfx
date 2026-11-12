use super::*;
use crate::{Circle, Rectangle};
use motiongfx::prelude::{ease, ms, s};

fn near(actual: Point, expected: Point) {
    assert!(actual.distance(expected) < 1e-9);
}

#[test]
fn camera_pan_zoom_rotation_render_and_reverse_seek() {
    let mut renderer = Renderer::default();
    let mut scene = Scene::new();
    let dot = scene.add(Circle::new(10.0));
    let camera = scene.camera();
    let mut animation = scene.animate(|sequence| {
        sequence.defaults(s(1), ease::linear);
        sequence.parallel(|sequence| {
            sequence.play(camera.position.to((100.0, 50.0)));
            sequence.play(camera.scale.to(3.0));
            sequence.play(
                camera.rotation.to(std::f64::consts::FRAC_PI_2),
            );
        });
        sequence.wait(s(1));
    });
    for time in [1000, 500, 2000, 0, 750] {
        animation.sample(ms(time));
        let t = (time as f64 / 1000.0).min(1.0);
        let view = animation.model().camera_transform();
        let center = Point::new(100.0 * t, -50.0 * t);
        near(view * center, Point::ZERO);
        let angle = t * std::f64::consts::FRAC_PI_2;
        let zoom = (1.0 + 2.0 * t).recip();
        near(
            view * (center + (1.0, 0.0)),
            Point::new(zoom * angle.cos(), zoom * angle.sin()),
        );

        // Compare actual submission to a directly transformed circle,
        // including the existing canvas fitting and DPI transforms.
        let viewport = crate::Size::new(400.0, 300.0);
        let canvas = crate::Size::new(200.0, 200.0);
        let mut actual = vello::Scene::new();
        renderer.render(
            &animation,
            &mut actual,
            viewport,
            canvas,
            2.0,
        );
        let mut expected = vello::Scene::new();
        animation.circle(dot).render(
            &mut expected,
            Affine::scale(2.0)
                * Affine::translate((200.0, 150.0))
                * Affine::scale(1.5)
                * view,
            1.0,
        );
        assert_eq!(
            actual.encoding().transforms,
            expected.encoding().transforms
        );
        assert_eq!(
            actual.encoding().path_data,
            expected.encoding().path_data
        );
    }
}

#[test]
fn scale_pan_approaches_target_monotonically_and_retraces_on_seek() {
    let scene = Scene::new();
    let camera = scene.camera();
    let mut animation = scene.animate(|sequence| {
        sequence.parallel(|sequence| {
            sequence.play(camera.scale.to(0.25).over(s(2)));
            sequence.play(camera.position.y.to(300.0).over(s(2)));
        });
        sequence.wait(s(1));
        sequence.parallel(|sequence| {
            sequence.play(camera.scale.to(1.0).over(s(2)));
            sequence.play(camera.position.y.to(0.0).over(s(2)));
        });
    });
    let tangent = Point::new(0.0, -300.0);
    let mut previous_distance = 300.0;
    for time in (0..=2000).step_by(50) {
        animation.sample(ms(time));
        let view = animation.model().camera_transform();
        let distance = -(view * tangent).y;
        assert!(distance >= -1e-9);
        assert!(distance <= previous_distance + 1e-9);
        previous_distance = distance;
    }
    // Midpoint: half-sized progress, not halfway magnification.
    // A 0.625 frame gives 1.6x magnification and a 240px offset.
    animation.sample(ms(1000));
    assert!((animation.camera().scale - 0.625).abs() < 1e-6);
    let midpoint = animation.model().camera_transform() * tangent;
    assert!((midpoint.y + 240.0).abs() < 1e-4);

    // The return leg traces the same views in reverse, including when
    // jumping across the hold or seeking backwards through both clips.
    for time in [0, 250, 1000, 1750, 2000, 500] {
        animation.sample(ms(time));
        let outward = animation.model().camera_transform();
        animation.sample(ms(5000 - time));
        let returning = animation.model().camera_transform();
        for point in [tangent, Point::new(70.0, 90.0)] {
            assert!(
                (outward * point).distance(returning * point) < 1e-3
            );
        }
    }
}

#[test]
fn follow_uses_nested_world_position_but_not_target_scale_or_rotation()
 {
    let mut scene = Scene::new();
    let parent = scene.add(Rectangle {
        position: Point::new(100.0, 50.0),
        scale: 2.0,
        rotation: std::f64::consts::FRAC_PI_2,
        ..Rectangle::default()
    });
    let target = scene.add(
        Circle::new(10.0)
            .relative_to(parent)
            .position((10.0, 0.0))
            .scale(3.0)
            .rotation(0.8),
    );
    let camera = scene.set_camera(
        Camera::new().relative_to(target).position((5.0, 6.0)),
    );
    let mut animation = scene.animate(|sequence| {
        sequence.defaults(s(1), ease::linear);
        sequence.parallel(|sequence| {
            sequence.play(parent.position.x.offset(100.0));
            sequence.play(target.position.x.to(20.0));
            sequence.play(target.scale.to(0.0));
            sequence.play(target.rotation.to(2.0));
            sequence.play(camera.position.x.to(15.0));
        });
    });
    for time in [0, 1000, 500, 0] {
        animation.sample(ms(time));
        let t = time as f64 / 1000.0;
        let center =
            Point::new(105.0 + 110.0 * t, -(76.0 + 20.0 * t));
        let view = animation.model().camera_transform();
        near(view * center, Point::ZERO);
        near(view * (center + (1.0, 0.0)), Point::new(1.0, 0.0));
    }
}

#[test]
#[should_panic(
    expected = "camera scale must be finite and greater than zero"
)]
fn invalid_scale_is_rejected_before_rendering() {
    let mut scene = Scene::new();
    scene.set_camera(Camera::new().scale(0.0));
    scene.model().camera_transform();
}
