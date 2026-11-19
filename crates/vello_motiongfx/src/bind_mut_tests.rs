//! Contracts for `bind_mut`: when it runs, its order, borrowing and
//! geometry reuse.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use vello::kurbo::{ParamCurve, QuadBez};

#[test]
fn bind_mut_runs_after_what_it_reads_and_only_when_that_changes() {
    let mut scene = Scene::new();
    let progress = signal(0.0);
    let a = scene.add(Circle::new(2.0));
    let b = scene.add(Circle::new(2.0));
    let line = scene.add(Line::new(Point::ZERO, Point::ZERO));
    let doubled = computed(&progress, |p| p * 2.0);
    scene.bind(a.position_x(), &doubled, |x| x);
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    // Reads the binding written in the same sample.
    scene.bind_mut(b, a.position(), move |b, position| {
        counter.fetch_add(1, Ordering::Relaxed);
        b.transform.position = position + (0.0, 30.0);
    });
    scene.bind_mut(
        line,
        (b.position(), &doubled),
        |line, (position, doubled)| {
            line.start = position;
            line.end = Point::new(doubled, 80.0);
        },
    );
    let time = scene.time();
    let clock_label = scene.add(Text::new(""));
    scene.bind_mut(clock_label, time, |label, time| {
        label.content = time.as_millis().to_string().into();
    });
    let mut animation = scene.compile([
        wait(s(1)),
        progress.to(10.0).over(s(2)).ease(ease::linear),
        wait(s(1)),
    ]);
    assert_eq!(calls.load(Ordering::Relaxed), 1); // The first sample runs it.
    // It runs again only when `a.position()` changes: not while `progress`
    // holds, nor when the same time is sampled twice.
    for (time, x, total) in [
        (0, 0.0, 1),
        (500, 0.0, 1),
        (2000, 10.0, 2),
        (2000, 10.0, 2),
        (4000, 20.0, 3),
        (1500, 5.0, 4),
        (0, 0.0, 5),
    ] {
        animation.sample(ms(time));
        assert_eq!(
            animation.get(b).transform.position,
            Point::new(x, 30.0)
        );
        assert_eq!(animation.get(line).start, Point::new(x, 30.0));
        assert_eq!(animation.get(line).end, Point::new(x, 80.0));
        assert_eq!(
            animation.get(clock_label).content.as_ref(),
            time.to_string()
        );
        assert_eq!(
            calls.load(Ordering::Relaxed),
            total,
            "at {time}ms"
        );
    }
    animation.sample(s(10));
    assert_eq!(animation.get(clock_label).content.as_ref(), "4000");
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
    scene.bind_mut((b, line, a), &amount, |(b, line, a), amount| {
        b.transform.position = Point::new(amount + 20.0, 0.0);
        a.transform.position = Point::new(amount + 10.0, 0.0);
        line.start = a.transform.position;
        line.end = b.transform.position;
    });
    let text = scene.add(Text::new(""));
    let rect = scene.add(Rectangle::new(10.0, 10.0));
    let camera = scene.add(Camera::new());
    scene.bind_mut(
        (rect, text, camera),
        &amount,
        |(rect, text, camera), amount| {
            rect.transform.rotation = amount;
            text.transform.position = rect.transform.position;
            camera.scale = 1.0 + amount;
        },
    );
    let mut animation =
        scene.compile([amount.to(1.0).ease(ease::linear)]);
    animation.sample(ms(500));
    assert_eq!(animation.get(line).start, Point::new(10.5, 0.0));
    assert_eq!(animation.get(line).end, Point::new(20.5, 0.0));
    assert_eq!(animation.get(rect).transform.rotation, 0.5);
    assert_eq!(animation.model().camera().scale, 1.5);
}

#[test]
fn array_targets_preserve_order_and_support_empty_arrays() {
    let mut scene = Scene::new();
    let amount = signal(0.0);
    let lines: [_; 3] = std::array::from_fn(|_| {
        scene.add(Line::new(Point::ZERO, Point::ZERO))
    });
    scene.bind_mut(
        [lines[2], lines[0], lines[1]],
        &amount,
        |lines, amount| {
            for (index, line) in lines.into_iter().enumerate() {
                line.end.x = amount + index as f64;
            }
        },
    );
    scene.bind_mut([] as [Handle<Line>; 0], &amount, |lines, _| {
        assert!(lines.is_empty())
    });
    let mut animation =
        scene.compile([amount.to(10.0).ease(ease::linear)]);
    animation.sample(ms(500));
    assert_eq!(animation.get(lines[2]).end.x, 5.0);
    assert_eq!(animation.get(lines[0]).end.x, 6.0);
    assert_eq!(animation.get(lines[1]).end.x, 7.0);
}

#[test]
#[should_panic(
    expected = "an update cannot borrow the same object twice"
)]
fn duplicate_targets_are_rejected_during_registration() {
    let mut scene = Scene::new();
    let dot = scene.add(Circle::new(2.0));
    scene.bind_mut((dot, dot), dot.radius(), |_, _| {});
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
    scene.bind_mut(curve, &bend, move |curve, bend| {
        curve.set_quadratic(start, (0.0, 170.0 - bend), end);
    });
    scene.bind_mut(
        (a, b, q),
        (&bend, &t),
        move |(a, b, q), (bend, t)| {
            let control = Point::new(0.0, 170.0 - bend);
            a.transform.position = start + (control - start) * t;
            b.transform.position = control + (end - control) * t;
            q.transform.position = a.transform.position
                + (b.transform.position - a.transform.position) * t;
        },
    );
    scene.bind_mut(
        line,
        (a.position(), b.position()),
        |line, (a, b)| {
            line.start = a;
            line.end = b;
        },
    );
    let mut animation = scene.compile([
        t.to(0.5).ease(ease::linear),
        chain(
            [200.0, -240.0, 40.0]
                .map(|offset| bend.offset(offset).ease(ease::linear)),
        ),
    ]);
    let initial_geometry = animation.get(curve).geometry.clone();
    animation.sample(ms(500));
    assert!(Arc::ptr_eq(
        &initial_geometry,
        &animation.get(curve).geometry
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
            animation.get(q).transform.position.distance(expected)
                < 1e-8
        );
        assert_eq!(
            animation.get(line).start,
            animation.get(a).transform.position
        );
        assert_eq!(
            animation.get(line).end,
            animation.get(b).transform.position
        );
        rendered.reset();
        let size = Size::new(1120.0, 800.0);
        renderer.render(&animation, &mut rendered, size, size, 1.0);
        let geometry = animation.get(curve).geometry.clone();
        let rebuilds =
            renderer.path_geometry[curve.id().index()].rebuilds;
        animation.sample(ms(time));
        rendered.reset();
        renderer.render(&animation, &mut rendered, size, size, 1.0);
        assert!(Arc::ptr_eq(
            &geometry,
            &animation.get(curve).geometry
        ));
        assert_eq!(
            renderer.path_geometry[curve.id().index()].rebuilds,
            rebuilds
        );
    }
}
