//! Contracts for typed callbacks, sample order, borrowing and geometry reuse.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use vello::kurbo::{ParamCurve, QuadBez};

#[test]
fn updates_run_after_bindings_in_registration_order_on_every_sample()
{
    let mut scene = Scene::new();
    let progress = signal(0.0);
    let a = scene.add(Circle::new(2.0));
    let b = scene.add(Circle::new(2.0));
    let line = scene.add(Line::new(Point::ZERO, Point::ZERO));
    let doubled = computed(&progress, |p| p * 2.0);
    scene.bind(a.position.x, &doubled, |x| x);
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    // Reads the just-sampled binding. This native callback itself is uncached.
    scene.update(b, a.position, move |b, position| {
        counter.fetch_add(1, Ordering::Relaxed);
        b.position = position + (0.0, 30.0);
    });
    scene.update(
        line,
        (b.position, &doubled),
        |line, (position, doubled)| {
            line.start = position;
            line.end = Point::new(doubled, 80.0);
        },
    );
    let time = scene.time();
    let clock_label = scene.add(Text::new(""));
    scene.update(clock_label, time, |label, time| {
        label.content = time.as_millis().to_string().into();
    });
    let mut animation = scene.animate(|sequence| {
        sequence.wait(s(1));
        sequence
            .play(progress.to(10.0).over(s(2)).ease(ease::linear));
        sequence.wait(s(1));
    });
    assert_eq!(calls.load(Ordering::Relaxed), 1); // Initializes at zero.
    for (index, (time, x)) in [
        (0, 0.0),
        (500, 0.0),
        (2000, 10.0),
        (2000, 10.0),
        (4000, 20.0),
        (1500, 5.0),
        (0, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        animation.sample(ms(time));
        assert_eq!(animation.circle(b).position, Point::new(x, 30.0));
        assert_eq!(animation.line(line).start, Point::new(x, 30.0));
        assert_eq!(animation.line(line).end, Point::new(x, 80.0));
        assert_eq!(
            animation.text(clock_label).content.as_ref(),
            time.to_string()
        );
        assert_eq!(calls.load(Ordering::Relaxed), index + 2);
    }
    animation.sample(s(10));
    assert_eq!(animation.text(clock_label).content.as_ref(), "4000");
}

#[test]
fn grouped_targets_preserve_caller_order_and_borrow_disjoint_objects()
{
    let mut scene = Scene::new();
    let amount = signal(0.0);
    let a = scene.add(Circle::new(1.0));
    let _unselected = scene.add(Circle::new(99.0));
    let b = scene.add(Circle::new(2.0));
    let line = scene.add(Line::new(Point::ZERO, Point::ZERO));
    // Reverse storage order and mix object types in one tuple.
    scene.update((b, line, a), &amount, |(b, line, a), amount| {
        b.position = Point::new(amount + 20.0, 0.0);
        a.position = Point::new(amount + 10.0, 0.0);
        line.start = a.position;
        line.end = b.position;
    });
    let text = scene.add(Text::new(""));
    let rect = scene.add(Rectangle::new(10.0, 10.0));
    let camera = scene.camera();
    scene.update(
        (rect, text, camera),
        &amount,
        |(rect, text, camera), amount| {
            rect.rotation = amount;
            text.position = rect.position;
            camera.scale = 1.0 + amount;
        },
    );
    let mut animation = scene.animate(|sequence| {
        sequence.play(amount.to(1.0).ease(ease::linear));
    });
    animation.sample(ms(500));
    assert_eq!(animation.line(line).start, Point::new(10.5, 0.0));
    assert_eq!(animation.line(line).end, Point::new(20.5, 0.0));
    assert_eq!(animation.rectangle(rect).rotation, 0.5);
    assert_eq!(animation.camera().scale, 1.5);
}

#[test]
fn array_targets_preserve_order_and_support_empty_arrays() {
    let mut scene = Scene::new();
    let amount = signal(0.0);
    let lines: [_; 3] = std::array::from_fn(|_| {
        scene.add(Line::new(Point::ZERO, Point::ZERO))
    });
    scene.update(
        [lines[2], lines[0], lines[1]],
        &amount,
        |lines, amount| {
            for (index, line) in lines.into_iter().enumerate() {
                line.end.x = amount + index as f64;
            }
        },
    );
    scene.update([] as [LineHandle; 0], &amount, |lines, _| {
        assert!(lines.is_empty())
    });
    let mut animation = scene.animate(|sequence| {
        sequence.play(amount.to(10.0).ease(ease::linear));
    });
    animation.sample(ms(500));
    assert_eq!(animation.line(lines[2]).end.x, 5.0);
    assert_eq!(animation.line(lines[0]).end.x, 6.0);
    assert_eq!(animation.line(lines[1]).end.x, 7.0);
}

#[test]
#[should_panic(
    expected = "an update cannot borrow the same object twice"
)]
fn duplicate_targets_are_rejected_during_registration() {
    let mut scene = Scene::new();
    let dot = scene.add(Circle::new(2.0));
    scene.update((dot, dot), dot.radius, |_, _| {});
}

#[test]
fn quadratic_updates_match_de_casteljau_reuse_geometry_and_rewind() {
    let mut renderer = Renderer::default();
    let mut scene = Scene::new();
    let bend = signal(0.0);
    let t = signal(0.0);
    let start = Point::new(-400.0, -150.0);
    let end = Point::new(400.0, -150.0);
    let curve = scene.add(Path::quadratic(start, (0.0, 170.0), end));
    let a = scene.add(Circle::new(1.0));
    let b = scene.add(Circle::new(1.0));
    let q = scene.add(Circle::new(1.0));
    let line = scene.add(Line::new(start, end));
    scene.update(curve, &bend, move |curve, bend| {
        curve.set_quadratic(start, (0.0, 170.0 - bend), end);
    });
    scene.update(
        (a, b, q),
        (&bend, &t),
        move |(a, b, q), (bend, t)| {
            let control = Point::new(0.0, 170.0 - bend);
            a.position = start + (control - start) * t;
            b.position = control + (end - control) * t;
            q.position = a.position + (b.position - a.position) * t;
        },
    );
    scene.update(line, (a.position, b.position), |line, (a, b)| {
        line.start = a;
        line.end = b;
    });
    let mut animation = scene.animate(|sequence| {
        sequence.play(t.to(0.5).ease(ease::linear));
        for offset in [200.0, -240.0, 40.0] {
            sequence.play(bend.offset(offset).ease(ease::linear));
        }
    });
    let initial_geometry = animation.path(curve).geometry.clone();
    animation.sample(ms(500));
    assert!(Arc::ptr_eq(
        &initial_geometry,
        &animation.path(curve).geometry
    ));
    let mut rendered = vello::Scene::new();
    for (time, expected_bend) in [
        (2000, 200.0),
        (3000, -40.0),
        (4000, 0.0),
        (2500, 80.0),
        (0, 0.0),
    ] {
        animation.sample(ms(time));
        let control = Point::new(0.0, 170.0 - expected_bend);
        let progress = (time as f64 / 1000.0).min(1.0) * 0.5;
        let expected =
            QuadBez::new(start, control, end).eval(progress);
        assert!(
            animation.circle(q).position.distance(expected) < 1e-8
        );
        assert_eq!(
            animation.line(line).start,
            animation.circle(a).position
        );
        assert_eq!(
            animation.line(line).end,
            animation.circle(b).position
        );
        rendered.reset();
        let size = Size::new(1120.0, 800.0);
        renderer.render(&animation, &mut rendered, size, size, 1.0);
        let geometry = animation.path(curve).geometry.clone();
        let rebuilds =
            renderer.path_geometry[curve.id().index()].rebuilds;
        animation.sample(ms(time));
        rendered.reset();
        renderer.render(&animation, &mut rendered, size, size, 1.0);
        assert!(Arc::ptr_eq(
            &geometry,
            &animation.path(curve).geometry
        ));
        assert_eq!(
            renderer.path_geometry[curve.id().index()].rebuilds,
            rebuilds
        );
    }
}
