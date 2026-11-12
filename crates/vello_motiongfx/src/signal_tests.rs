//! Source-aware authoring contracts, independent of example choreography.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use vello::kurbo::{ParamCurve, QuadBez};

#[test]
fn shared_computed_sources_drive_builders_and_skip_unchanged_inputs()
{
    let mut scene = Scene::new();
    let progress = scene.signal(0.0);
    let radius = scene.signal(10.0);
    let fill = scene.signal(colors::BLUE);
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    // Tuple output converts to a Point in position/start/end builders.
    let position =
        scene.computed((progress, radius), move |(p, r)| {
            counter.fetch_add(1, Ordering::Relaxed);
            (p * r, r)
        });
    let opacity = scene.computed(progress, |p| 1.0 - p);
    let descriptor = Circle::new(radius)
        .position(position)
        .fill(fill)
        .opacity(opacity);
    let a = scene.add(descriptor.clone());
    let b = scene.add(descriptor);
    let line = scene.add(
        Line::new((0.0, 0.0), position)
            .stroke(fill, 2.0)
            .opacity(opacity),
    );
    let mut animation = scene.animate(|sequence| {
        sequence.parallel(|sequence| {
            sequence.play(progress.to(1.0).ease(ease::linear));
            sequence.play(radius.to(20.0).ease(ease::linear));
            sequence.play(fill.to(colors::ORANGE).ease(ease::linear));
        });
        sequence.wait(s(1));
    });
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    for (time, p, r, expected_calls) in [
        (0, 0.0, 10.0, 1),
        (500, 0.5, 15.0, 2),
        (500, 0.5, 15.0, 2),
        (2000, 1.0, 20.0, 3),
        (1500, 1.0, 20.0, 3),
        (0, 0.0, 10.0, 4),
    ] {
        animation.sample(ms(time));
        let expected = Point::new(p * r, r);
        assert_eq!(animation.circle(a).position, expected);
        assert_eq!(animation.circle(b).position, expected);
        assert_eq!(animation.circle(a).radius, r);
        assert_eq!(animation.circle(a).opacity, 1.0 - p);
        assert_eq!(animation.line(line).end, expected);
        assert_eq!(
            animation.line(line).stroke,
            animation.circle(a).fill
        );
        assert_eq!(calls.load(Ordering::Relaxed), expected_calls);
    }
}

#[test]
fn point_signals_discrete_arrays_text_and_camera_use_the_same_source_model()
 {
    let mut scene = Scene::new();
    let center = scene.signal(Point::ZERO);
    let points = scene
        .signal([Point::new(10.0, 20.0), Point::new(30.0, 40.0)]);
    let visible = scene.signal(false); // No boolean interpolation is required.
    let opacity = scene
        .computed(visible, |visible| if visible { 1.0 } else { 0.0 });
    let selected = scene.computed(points, |points| points[1]);
    let content =
        scene.computed((center, points), |(center, points)| {
            Arc::<str>::from(format!("{} {}", center.x, points[0].x))
        });
    let text = scene
        .add(Text::new(content).position(center).opacity(opacity));
    let line = scene.add(Line::new(center, selected));
    let size =
        scene.computed(center, |p| Size::new(10.0 + p.x, 20.0));
    let rect = scene.add(Rectangle::new(10.0, 20.0).size(size));
    scene.set_camera(Camera::new().position(center));
    let mut animation = scene.animate(|sequence| {
        sequence.play(center.to((100.0, 50.0)).ease(ease::linear));
        sequence.play(visible.set(true));
        sequence.play(
            points.set([
                Point::new(50.0, 60.0),
                Point::new(70.0, 80.0),
            ]),
        );
        sequence.wait(s(1));
    });
    for (time, x, point_x, alpha) in [
        (0, 0.0, 30.0, 0.0),
        (500, 50.0, 30.0, 0.0),
        (2000, 100.0, 70.0, 1.0),
        (0, 0.0, 30.0, 0.0),
    ] {
        animation.sample(ms(time));
        assert_eq!(animation.text(text).position.x, x);
        assert_eq!(animation.text(text).opacity, alpha);
        assert_eq!(animation.line(line).end.x, point_x);
        assert_eq!(animation.rectangle(rect).size.width, 10.0 + x);
        assert_eq!(animation.camera().position.x, x);
        assert_eq!(
            animation.text(text).content.as_ref(),
            format!("{x} {}", point_x - 20.0)
        );
    }
}

#[test]
fn source_driven_quadratic_matches_math_and_preserves_render_caches()
{
    let mut scene = Scene::new();
    let bend = scene.signal(0.0);
    let t = scene.signal(0.0);
    let start = Point::new(-400.0, -150.0);
    let end = Point::new(400.0, -150.0);
    let control =
        scene.computed(bend, |bend| Point::new(0.0, 170.0 - bend));
    let position = scene
        .computed((control, t), move |(control, t)| {
            QuadBez::new(start, control, end).eval(t)
        });
    let curve = scene.add(Path::quadratic(start, control, end));
    let dot = scene.add(Circle::new(6.0).position(position));
    let mut animation = scene.animate(|sequence| {
        sequence.play(t.to(0.5).ease(ease::linear));
        for offset in [200.0, -240.0, 40.0] {
            sequence.play(bend.offset(offset).ease(ease::linear));
        }
    });
    let initial = animation.path(curve).geometry.clone();
    let mut rendered = vello::Scene::new();
    for (time, bend) in [
        (500, 0.0),
        (2000, 200.0),
        (3000, -40.0),
        (4000, 0.0),
        (2500, 80.0),
        (0, 0.0),
    ] {
        animation.sample(ms(time));
        if time == 500 {
            assert!(Arc::ptr_eq(
                &initial,
                &animation.path(curve).geometry
            ));
        }
        let t = (time as f64 / 1000.0).min(1.0) * 0.5;
        let geometry = &animation.path(curve).geometry;
        let actual = geometry.segments().next().unwrap().eval(t);
        let expected =
            QuadBez::new(start, Point::new(0.0, 170.0 - bend), end)
                .eval(t);
        assert!(actual.distance(expected) < 1e-8);
        assert!(
            animation.circle(dot).position.distance(expected) < 1e-8
        );
        let size = Size::new(1120.0, 800.0);
        rendered.reset();
        animation.render(&mut rendered, size, size, 1.0);
        let source = animation.path(curve).geometry.clone();
        let rebuilds = animation.path_geometry[curve.id.0].rebuilds;
        animation.sample(ms(time));
        rendered.reset();
        animation.render(&mut rendered, size, size, 1.0);
        assert!(Arc::ptr_eq(
            &source,
            &animation.path(curve).geometry
        ));
        assert_eq!(
            animation.path_geometry[curve.id.0].rebuilds,
            rebuilds
        );
    }
}

#[test]
fn repeated_setters_replace_wiring_and_source_aware_fields_reject_timeline_writers()
 {
    let mut scene = Scene::new();
    let radius = scene.signal(10.0);
    // Last setter wins; obsolete wiring must not survive a fixed override.
    let dot = scene.add(Circle::new(radius).radius(5.0));
    let mut animation = scene
        .try_animate(|sequence| {
            sequence.play(dot.radius.to(20.0).ease(ease::linear));
        })
        .unwrap();
    animation.sample(ms(500));
    assert_eq!(animation.circle(dot).radius, 12.5);

    let mut scene = Scene::new();
    let radius = scene.signal(10.0);
    let dot = scene.add(Circle::new(radius));
    assert!(matches!(
        scene.try_animate(|sequence| {
            sequence.play(dot.radius.to(20.0));
        }),
        Err(BindingError::AnimatedDestination(_))
    ));

    let mut first = Scene::new();
    let source = first.signal(0.0);
    let foreign = first.computed(source, |v| (v, v));
    let mut second = Scene::new();
    second.add(Circle::new(1.0).position(foreign));
    assert!(matches!(
        second.try_animate(|_| {}),
        Err(BindingError::ForeignDerived)
    ));
}

#[test]
fn line_reveal_uses_interpolated_endpoint_without_a_path_buffer() {
    let mut scene = Scene::new();
    let reveal = scene.signal(0.0);
    scene.add(
        Line::new((0.0, 10.0), (80.0, 50.0))
            .stroke(colors::BLUE, 2.0)
            .reveal(reveal),
    );
    let mut animation = scene.animate(|sequence| {
        sequence.play(reveal.to(1.0).ease(ease::linear));
    });
    let size = Size::new(200.0, 200.0);
    for (time, end) in [
        (0, None),
        (500, Some((40.0, -30.0))),
        (1000, Some((80.0, -50.0))),
        (0, None),
    ] {
        animation.sample(ms(time));
        let mut actual = vello::Scene::new();
        animation.render(&mut actual, size, size, 1.0);
        let mut expected = vello::Scene::new();
        if let Some(end) = end {
            expected.stroke(
                &Stroke::new(2.0),
                Affine::translate((100.0, 100.0)),
                colors::BLUE,
                None,
                &vello::kurbo::Line::new((0.0, -10.0), end),
            );
        }
        assert_eq!(
            actual.encoding().path_data,
            expected.encoding().path_data
        );
        assert_eq!(
            actual.encoding().transforms,
            expected.encoding().transforms
        );
        assert_eq!(
            actual.encoding().draw_data,
            expected.encoding().draw_data
        );
    }
}
