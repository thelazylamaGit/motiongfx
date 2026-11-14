//! Geometry, attachment and seeking regressions for typed endpoint shapes.
use super::*;
use vello::kurbo::{Line as LineShape, PathEl};

#[test]
fn stroke_shapes_send_configured_caps_and_joins_to_vello() {
    let mut renderer = Renderer::default();
    for cap in [Cap::Butt, Cap::Square, Cap::Round] {
        for join in [Join::Miter, Join::Bevel, Join::Round] {
            let mut scene = Scene::new();
            macro_rules! add {
                ($object:expr) => {
                    scene.add(
                        $object
                            .stroke(Color::WHITE, 3.0)
                            .line_cap(cap)
                            .line_join(join),
                    );
                };
            }
            add!(Line::new((0.0, 0.0), (100.0, 0.0)));
            add!(Path::new([(0.0, 0.0), (20.0, 20.0), (40.0, 0.0)]));
            add!(Triangle::new().fill(Color::TRANSPARENT));
            add!(Chevron::new());
            add!(Rectangle::new(20.0, 20.0).fill(Color::TRANSPARENT));
            let animation = scene.compile([wait(s(1))]);
            let mut actual = vello::Scene::new();
            let size = Size::new(200.0, 200.0);
            renderer.render(&animation, &mut actual, size, size, 1.0);
            let mut expected = vello::Scene::new();
            for _ in 0..5 {
                expected.stroke(
                    &Stroke::new(3.0).with_caps(cap).with_join(join),
                    Affine::IDENTITY,
                    Color::WHITE,
                    None,
                    &LineShape::new((0.0, 0.0), (100.0, 0.0)),
                );
            }
            let strokes: Vec<_> = actual
                .encoding()
                .styles
                .iter()
                .filter(|style| style.line_width > 0.0)
                .collect();
            assert!(!strokes.is_empty());
            assert!(strokes.iter().all(
                |style| **style == expected.encoding().styles[0]
            ));
        }
    }
}

#[test]
fn reactive_caps_and_tip_joins_are_independent_discrete_seekable_styles()
 {
    let mut renderer = Renderer::default();
    let cap = signal(Cap::Butt);
    let mut scene = Scene::new();
    let line = scene.add(
        Line::new((0.0, 0.0), (200.0, 0.0))
            .end_tip(
                Chevron::new()
                    .stroke(Color::WHITE, 4.0)
                    .line_cap(Cap::Round)
                    .line_join(Join::Miter),
            )
            .stroke(Color::WHITE, 3.0)
            .line_cap(&cap)
            .line_join(Join::Bevel),
    );
    let mut animation = scene.compile([
        wait(s(1)),
        cap.set(Cap::Square),
        line.end_tip.line_join().set(Join::Round),
        wait(s(1)),
    ]);
    let size = Size::new(400.0, 200.0);
    for (time, cap, join) in [
        (0, Cap::Butt, Join::Miter),
        (1500, Cap::Square, Join::Round),
        (0, Cap::Butt, Join::Miter),
    ] {
        animation.sample(ms(time));
        assert_eq!(animation.line(line).line_cap, cap);
        assert_eq!(animation.chevron(line.end_tip).line_join, join);
        let mut actual = vello::Scene::new();
        renderer.render(&animation, &mut actual, size, size, 1.0);
        let mut expected = vello::Scene::new();
        for stroke in [
            Stroke::new(3.0).with_caps(cap).with_join(Join::Bevel),
            Stroke::new(4.0).with_caps(Cap::Round).with_join(join),
        ] {
            expected.stroke(
                &stroke,
                Affine::IDENTITY,
                Color::WHITE,
                None,
                &LineShape::new((0.0, 0.0), (200.0, 0.0)),
            );
        }
        assert_eq!(
            actual.encoding().styles,
            expected.encoding().styles
        );
    }
}

#[test]
fn replacement_tips_drop_obsolete_wiring_and_grouped_updates_borrow_shapes_safely()
 {
    let mut scene = Scene::new();
    let amount = signal(0.0);
    let line = scene.add(
        Line::new((0.0, 0.0), (100.0, 0.0))
            .start_tip(Circle::new(amount.map(|v| v + 5.0)))
            .start_tip(Triangle::new())
            .end_tip(Chevron::new()),
    );
    assert!(scene.model().circles().is_empty());
    assert_eq!(scene.model().draw_order().len(), 1);
    // The line and its registered tips use disjoint storage. Their ordinary
    // typed handles also remain usable as a heterogeneous update target tuple.
    scene.update(
        (line, line.start_tip, line.end_tip),
        &amount,
        |(line, triangle, chevron), value| {
            line.end.y = value * 100.0;
            triangle.angle = 0.6 + value * 0.2;
            chevron.stroke_width = 2.0 + value * 4.0;
        },
    );
    let mut animation =
        scene.compile([amount.to(1.0).ease(ease::linear)]);
    for (time, value) in [(500, 0.5), (1000, 1.0), (0, 0.0)] {
        animation.sample(ms(time));
        assert_eq!(animation.line(line).end.y, value * 100.0);
        assert_eq!(
            animation.triangle(line.start_tip).angle,
            0.6 + value * 0.2
        );
        assert_eq!(
            animation.chevron(line.end_tip).stroke_width,
            2.0 + value * 4.0
        );
    }
}

#[test]
fn dashed_tips_follow_trimmed_revealed_endpoints_in_both_directions()
{
    let mut renderer = Renderer::default();
    for (end, shaft, tips, angle) in [
        (
            (110.0, 20.0),
            [(27.0, -20.0), (46.5, -20.0)],
            [(15.0, -20.0), (58.5, -20.0)],
            0.0,
        ),
        (
            (10.0, 120.0),
            [(10.0, -37.0), (10.0, -56.5)],
            [(10.0, -25.0), (10.0, -68.5)],
            std::f64::consts::FRAC_PI_2,
        ),
    ] {
        let mut scene = Scene::new();
        let tip = Triangle::new()
            .size(12.0)
            .angle(std::f64::consts::FRAC_PI_2)
            .fill(colors::BLUE);
        scene.add(
            Line::new((10.0, 20.0), end)
                .stroke(colors::BLUE, 4.0)
                .line_dash([20.0, 20.0])
                .dash_offset(3.0)
                .start_tip(tip.clone())
                .end_tip(tip)
                .start_offset(5.0)
                .end_offset(8.0)
                .reveal(0.5)
                .opacity(0.5),
        );
        let animation = scene.compile([wait(s(1))]);
        let mut actual = vello::Scene::new();
        let size = Size::new(200.0, 200.0);
        renderer.render(&animation, &mut actual, size, size, 1.0);
        let mut expected = vello::Scene::new();
        let xf = Affine::translate((100.0, 100.0));
        let color = colors::BLUE.with_alpha(0.5);
        expected.stroke(
            &Stroke::new(4.0).with_dashes(3.0, [20.0, 20.0]),
            xf,
            color,
            None,
            &LineShape::new(shaft[0], shaft[1]),
        );
        let half_width =
            12.0 * (std::f64::consts::FRAC_PI_2 / 2.0).tan();
        let geometry = [
            PathEl::MoveTo((-12.0, -half_width).into()),
            PathEl::LineTo((0.0, -0.0).into()),
            PathEl::LineTo((-12.0, half_width).into()),
            PathEl::ClosePath,
        ];
        for (position, rotation) in [
            (tips[0], angle + std::f64::consts::PI),
            (tips[1], angle),
        ] {
            expected.fill(
                Fill::NonZero,
                xf * Affine::translate(position)
                    * Affine::rotate(-rotation),
                color,
                None,
                &geometry,
            );
        }
        assert_eq!(
            actual.encoding().draw_data,
            expected.encoding().draw_data
        );
        assert_eq!(
            actual.encoding().path_data,
            expected.encoding().path_data
        );
        assert_eq!(
            actual.encoding().transforms,
            expected.encoding().transforms
        );
    }
}

#[test]
fn degenerate_or_fully_trimmed_tips_emit_no_geometry() {
    let mut renderer = Renderer::default();
    for (start, end, trim) in [
        ((0.0, 0.0), (0.0, 0.0), 0.0),
        ((0.0, 0.0), (10.0, 0.0), 20.0),
    ] {
        let mut scene = Scene::new();
        scene.add(
            Line::new(start, end)
                .start_tip(Triangle::new())
                .end_tip(Circle::new(5.0))
                .end_offset(trim),
        );
        let animation = scene.compile([wait(s(1))]);
        let mut rendered = vello::Scene::new();
        let size = Size::new(100.0, 100.0);
        renderer.render(&animation, &mut rendered, size, size, 1.0);
        assert_eq!(rendered.encoding().n_paths, 0);
    }
}

#[test]
fn attached_shapes_retain_typed_reactive_properties_and_seek_independently()
 {
    let radius = signal(4.0);
    let mut scene = Scene::new();
    let line = scene.add(
        Line::new((0.0, 0.0), (100.0, 0.0))
            .start_tip(Circle::new(&radius).fill(colors::RED))
            .end_tip(
                Triangle::new()
                    .size(20.0)
                    .angle(0.6)
                    .fill(colors::BLUE),
            ),
    );
    // Attachments share shape storage, but never add independent draw entries.
    assert_eq!(scene.model().draw_order().len(), 1);
    assert_eq!(scene.model().circles().len(), 1);
    assert_eq!(scene.model().triangles().len(), 1);
    let mut animation = scene.compile([all([
        radius.to(8.0).ease(ease::linear),
        line.end_tip.angle().to(1.0).ease(ease::linear),
        line.end_tip.size().to(30.0).ease(ease::linear),
        line.end_tip.fill().to(colors::ORANGE).ease(ease::linear),
        line.end().to_xy(0.0, 200.0).ease(ease::linear),
    ])]);
    for (time, r, size, angle, end) in [
        (500, 6.0, 25.0, 0.8, Point::new(50.0, 100.0)),
        (1000, 8.0, 30.0, 1.0, Point::new(0.0, 200.0)),
        (0, 4.0, 20.0, 0.6, Point::new(100.0, 0.0)),
    ] {
        animation.sample(ms(time));
        assert_eq!(animation.circle(line.start_tip).radius, r);
        let tip = animation.triangle(line.end_tip);
        assert!((tip.size - size).abs() < 1e-6);
        assert!((tip.angle - angle).abs() < 1e-6);
        assert_eq!(animation.line(line).end, end);
    }
    assert_eq!(animation.triangle(line.end_tip).fill, colors::BLUE);
}

#[test]
fn tip_renderer_matches_standalone_shape_and_inherits_line_opacity_once()
 {
    let mut renderer = Renderer::default();
    // With no shaft, the endpoint triangle must produce exactly the same
    // commands as a standalone triangle positioned/oriented at that endpoint.
    let triangle = Triangle::new()
        .size(12.0)
        .angle(0.6)
        .radius(2.0)
        .fill(colors::RED)
        .stroke(colors::BLUE, 2.0)
        .opacity(0.5);
    let mut attached = Scene::new();
    attached.add(
        Line::new((0.0, 0.0), (100.0, 50.0))
            .stroke(colors::WHITE, 0.0)
            .end_tip(triangle.clone())
            .opacity(0.5),
    );
    let mut standalone = Scene::new();
    standalone.add(
        triangle
            .position((100.0, 50.0))
            .rotation(50.0_f64.atan2(100.0))
            .opacity(0.25),
    );
    let a = attached.compile([wait(s(1))]);
    let b = standalone.compile([wait(s(1))]);
    let size = Size::new(300.0, 200.0);
    let mut actual = vello::Scene::new();
    let mut expected = vello::Scene::new();
    renderer.render(&a, &mut actual, size, size, 1.0);
    renderer.render(&b, &mut expected, size, size, 1.0);
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

#[test]
fn rounded_tip_radius_is_reactive_seekable_and_geometry_cache_ignores_style_changes()
 {
    let mut renderer = Renderer::default();
    let radius = signal(0.0);
    let mut scene = Scene::new();
    let line =
        scene.add(Line::new((0.0, 0.0), (200.0, 0.0)).end_tip(
            Triangle::new().radius(&radius).fill(colors::RED),
        ));
    let mut animation = scene.compile([
        radius.to(4.0).ease(ease::linear),
        all([
            line.end_tip.fill().to(colors::BLUE),
            line.end_tip.rotation().to(0.3),
        ]),
    ]);
    let mut rendered = vello::Scene::new();
    let size = Size::new(400.0, 200.0);
    for (time, radius, rebuilds) in [
        (0, 0.0, 1),
        (500, 2.0, 2),
        (500, 2.0, 2),
        (1000, 4.0, 3),
        (1500, 4.0, 3),
        (0, 0.0, 4),
    ] {
        animation.sample(ms(time));
        rendered.reset();
        renderer.render(&animation, &mut rendered, size, size, 1.0);
        assert_eq!(animation.triangle(line.end_tip).radius, radius);
        let cache =
            &renderer.triangle_geometry[line.end_tip.id().index()];
        assert_eq!(cache.rebuilds, rebuilds);
        assert_eq!(
            cache
                .geometry
                .elements()
                .iter()
                .any(|el| matches!(el, PathEl::CurveTo(..))),
            radius > 0.0
        );
    }
    // Direct typed property animations use the same path as reactive radius.
    let mut scene = Scene::new();
    let triangle = scene.add(Triangle::new());
    let mut animation =
        scene.compile([triangle.radius().to(6.0).ease(ease::linear)]);
    animation.sample(ms(500));
    assert_eq!(animation.triangle(triangle).radius, 3.0);
}

#[test]
fn short_lines_fit_tips_and_open_chevrons_keep_the_shaft_to_the_apex()
{
    let mut renderer = Renderer::default();
    let mut scene = Scene::new();
    let line = scene.add(
        Line::new((0.0, 0.0), (10.0, 0.0))
            .start_tip(Circle::new(20.0))
            .end_tip(Triangle::new().size(20.0)),
    );
    let animation = scene.compile([wait(s(1))]);
    let mut rendered = vello::Scene::new();
    let size = Size::new(100.0, 100.0);
    renderer.render(&animation, &mut rendered, size, size, 1.0);
    // Both tips fit at 1/4 scale; the shaft vanishes instead of reversing.
    assert_eq!(rendered.encoding().n_paths, 2);
    assert_eq!(animation.circle(line.start_tip).radius, 20.0); // State isn't mutated to fit.

    let mut scene = Scene::new();
    let line =
        scene.add(Line::new((0.0, 0.0), (100.0, 0.0)).end_tip(
            Chevron::new().size(20.0).stroke(colors::ORANGE, 3.0),
        ));
    let animation = scene.compile([line.end_tip.angle().to(0.9)]);
    rendered.reset();
    renderer.render(&animation, &mut rendered, size, size, 1.0);
    let mut expected = vello::Scene::new();
    expected.stroke(
        &Stroke::new(1.0),
        Affine::translate((50.0, 50.0)),
        Color::WHITE,
        None,
        &LineShape::new((0.0, -0.0), (100.0, -0.0)),
    );
    assert_eq!(
        &rendered.encoding().path_data
            [..expected.encoding().path_data.len()],
        expected.encoding().path_data
    );
    assert_eq!(rendered.encoding().n_paths, 2); // One shaft, one open outline; no fill.
}
