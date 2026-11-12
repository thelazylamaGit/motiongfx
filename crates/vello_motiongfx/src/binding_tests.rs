//! Contracts for native bindings and reusable line primitives.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use vello::kurbo::ParamCurve;

#[test]
fn chained_bindings_sort_dependencies_cache_inputs_and_seek() {
    let mut scene = Scene::new();
    let source = scene.add(Circle::new(10.0));
    let dot = scene.add(Circle::new(2.0));
    let connector = scene.add(Line::new(Point::ZERO, Point::ZERO));
    // Register consumers before their producer. Whole-point and axis reads
    // both depend on the writer of dot.position, regardless of author order.
    scene.bind(connector.start, dot.position, |position| position);
    scene.bind(
        connector.end,
        (source.radius, dot.position.y),
        |(r, y)| Point::new(r, y),
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    scene.bind(dot.position, source.radius, move |radius| {
        counter.fetch_add(1, Ordering::Relaxed);
        Point::new(radius * 2.0, radius * 3.0)
    });
    let mut animation = scene
        .try_animate(|sequence| {
            sequence.wait(s(1));
            sequence.play(
                source.radius.to(20.0).over(s(2)).ease(ease::linear),
            );
            sequence.wait(s(1));
        })
        .unwrap();
    assert_eq!(animation.duration(), s(4));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    // Includes both holds, repeated samples, forward jumps and backward seeks.
    for (time, radius, expected_calls) in [
        (0, 10.0, 1),
        (500, 10.0, 1),
        (2000, 15.0, 2),
        (2000, 15.0, 2),
        (4000, 20.0, 3),
        (3500, 20.0, 3),
        (1500, 12.5, 4),
        (0, 10.0, 5),
    ] {
        animation.sample(ms(time));
        let position = Point::new(radius * 2.0, radius * 3.0);
        assert_eq!(animation.circle(dot).position, position);
        assert_eq!(animation.line(connector).start, position);
        assert_eq!(
            animation.line(connector).end,
            Point::new(radius, position.y)
        );
        assert_eq!(calls.load(Ordering::Relaxed), expected_calls);
    }
}

#[test]
fn shared_derived_values_compute_once_and_support_offsets_and_multiple_inputs()
 {
    let mut scene = Scene::new();
    let source = scene.add(Circle::new(10.0));
    let dot = scene.add(Circle::new(2.0));
    let line = scene.add(Line::new(Point::ZERO, Point::ZERO));
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let shared = scene.computed(
        (source.position, source.radius),
        move |(position, radius)| {
            counter.fetch_add(1, Ordering::Relaxed);
            position + (radius, 0.0)
        },
    );
    let offset =
        scene.computed(shared, |position| position + (0.0, 30.0));
    scene.bind(dot.position, shared, |position| position);
    scene.bind(line.start, shared, |position| position);
    scene.bind(line.end, offset, |position| position);
    let mut animation = scene
        .try_animate(|sequence| {
            sequence.parallel(|sequence| {
                sequence.play(
                    source.position.x.to(20.0).ease(ease::linear),
                );
                sequence
                    .play(source.radius.to(30.0).ease(ease::linear));
            });
        })
        .unwrap();
    for (time, expected, count) in [
        (0, 10.0, 1),
        (500, 30.0, 2),
        (500, 30.0, 2),
        (1000, 50.0, 3),
        (0, 10.0, 4),
    ] {
        animation.sample(ms(time));
        assert_eq!(
            animation.circle(dot).position,
            Point::new(expected, 0.0)
        );
        assert_eq!(
            animation.line(line).start,
            Point::new(expected, 0.0)
        );
        assert_eq!(
            animation.line(line).end,
            Point::new(expected, 30.0)
        );
        assert_eq!(calls.load(Ordering::Relaxed), count);
    }
}

#[test]
fn binding_validation_rejects_feedback_and_cycles_through_derived_values()
 {
    let mut scene = Scene::new();
    let dot = scene.add(Circle::new(2.0));
    scene.bind(dot.position, dot.position.x, |x| {
        Point::new(x + 1.0, 0.0)
    });
    assert!(matches!(
        scene.try_animate(|_| {}),
        Err(BindingError::Cycle)
    ));

    let mut scene = Scene::new();
    let a = scene.add(Circle::new(2.0));
    let b = scene.add(Circle::new(2.0));
    let derived =
        scene.computed(b.position, |position| position + (1.0, 0.0));
    scene.bind(a.position, derived, |position| position);
    scene.bind(b.position, a.position, |position| position);
    assert!(matches!(
        scene.try_animate(|_| {}),
        Err(BindingError::Cycle)
    ));
}

#[test]
fn binding_validation_rejects_overlapping_writers_and_timeline_actions()
 {
    let mut scene = Scene::new();
    let dot = scene.add(Circle::new(2.0));
    scene.bind(dot.position, dot.radius, |r| Point::new(r, 0.0));
    scene.bind(dot.position.x, dot.radius, |r| r);
    assert!(matches!(
        scene.try_animate(|_| {}),
        Err(BindingError::ConflictingWriters(_))
    ));

    let mut scene = Scene::new();
    let dot = scene.add(Circle::new(2.0));
    scene.bind(dot.position, dot.radius, |r| Point::new(r, 0.0));
    // Even raw action fragments participate in writer validation.
    let result = scene.try_animate(|sequence| {
        let change = sequence.action(dot.position.y.to(10.0));
        sequence.append(change);
    });
    assert!(matches!(
        result,
        Err(BindingError::AnimatedDestination(_))
    ));

    // Sibling axes are independent writers. Different subjects are too.
    let mut scene = Scene::new();
    let a = scene.add(Circle::new(2.0));
    let b = scene.add(Circle::new(2.0));
    scene.bind(a.position.x, a.radius, |r| r);
    scene.bind(a.position.y, a.radius, |r| r * 2.0);
    assert!(
        scene
            .try_animate(|sequence| {
                sequence.play(b.position.x.to(30.0));
            })
            .is_ok()
    );
}

#[test]
fn binding_validation_rejects_foreign_derived_values_and_missing_properties()
 {
    let mut first = Scene::new();
    let a = first.add(Circle::new(2.0));
    let shared = first.computed(a.radius, |r| r * 2.0);
    let mut second = Scene::new();
    let b = second.add(Circle::new(2.0));
    second.bind(b.radius, shared, |r| r);
    assert!(matches!(
        second.try_animate(|_| {}),
        Err(BindingError::ForeignDerived)
    ));

    let mut second = Scene::new();
    let rectangle = second.add(Rectangle::new(20.0, 20.0));
    second.bind(rectangle.position.x, a.radius, |r| r);
    assert!(matches!(
        second.try_animate(|_| {}),
        Err(BindingError::MissingProperty(_))
    ));
}

#[test]
fn sine_geometry_and_bound_endpoints_agree_when_scrubbing() {
    let mut scene = Scene::new();
    let turns = 2.125;
    let curve = scene.add(
        Path::new((0..=1088).map(|i| {
            let t = turns * i as f64 / 1088.0;
            Point::new(
                -240.0 + t * 320.0,
                80.0 * (t * std::f64::consts::TAU).sin(),
            )
        }))
        .end(0.0),
    );
    let dot = scene.add(Circle::new(6.4));
    let line = scene.add(Line::new(Point::ZERO, Point::ZERO));
    scene.bind(dot.position, curve.end, move |progress| {
        let angle = progress * turns * std::f64::consts::TAU;
        Point::new(-320.0 + 80.0 * angle.cos(), 80.0 * angle.sin())
    });
    scene.bind(line.start, dot.position, |p| p);
    scene.bind(
        line.end,
        (curve.end, dot.position.y),
        move |(p, y)| Point::new(-240.0 + p * turns * 320.0, y),
    );
    let mut animation = scene
        .try_animate(|sequence| {
            sequence.play(
                curve.end.to(1.0).over(ms(8500)).ease(ease::linear),
            );
            sequence.wait(s(1));
        })
        .unwrap();
    let mut rendered = vello::Scene::new();
    for time in [2000, 9500, 4250, 8500, 0, 1000] {
        animation.sample(ms(time));
        rendered.reset();
        animation.render(
            &mut rendered,
            Size::new(1120.0, 600.0),
            Size::new(1120.0, 600.0),
            1.0,
        );
        assert_eq!(
            animation.line(line).start,
            animation.circle(dot).position
        );
        let visible = &animation.path_geometry[curve.id.0].visible;
        if time == 0 {
            assert!(visible.is_empty());
        } else {
            let tip = visible.segments().last().unwrap().eval(1.0);
            assert!(tip.distance(animation.line(line).end) < 0.003);
        }
    }
    animation.sample(s(2));
    assert!(
        animation
            .circle(dot)
            .position
            .distance(Point::new(-400.0, 0.0))
            < 1e-4
    );
}

#[test]
fn line_rendering_matches_direct_vello_with_camera_and_animated_endpoints()
 {
    let mut scene = Scene::new();
    let line = scene.add(
        Line::new((-30.0, -10.0), (20.0, 40.0))
            .stroke(colors::BLUE, 3.0),
    );
    scene.set_camera(Camera::new().scale(0.5).position((20.0, 30.0)));
    let mut animation = scene.animate(|sequence| {
        sequence.parallel(|sequence| {
            sequence.play(line.start.x.to(10.0).ease(ease::linear));
            sequence
                .play(line.end.to((60.0, 80.0)).ease(ease::linear));
        });
    });
    animation.sample(ms(500));
    let mut actual = vello::Scene::new();
    animation.render(
        &mut actual,
        Size::new(200.0, 200.0),
        Size::new(200.0, 200.0),
        1.0,
    );
    let mut expected = vello::Scene::new();
    expected.stroke(
        &Stroke::new(3.0),
        Affine::translate((100.0, 100.0))
            * Affine::scale(2.0)
            * Affine::translate((-20.0, 30.0)),
        colors::BLUE,
        None,
        &vello::kurbo::Line::new((-10.0, 10.0), (40.0, -60.0)),
    );
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
}
