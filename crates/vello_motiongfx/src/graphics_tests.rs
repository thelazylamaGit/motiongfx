//! Regression tests for reusable primitives, not scene-specific choreography.

use super::*;
use vello::kurbo::{Circle as CircleShape, ParamCurve};

#[test]
fn revealed_path_is_seekable_and_movement_does_not_rebuild_geometry()
{
    let mut scene = Scene::new();
    let path = scene.add(
        Path::new([(0.0, 0.0), (10.0, 0.0), (30.0, 0.0)]).end(0.0),
    );
    let mut animation = scene.animate(|sequence| {
        sequence.defaults(s(1), ease::linear);
        sequence.parallel(|sequence| {
            sequence.play(path.end.to(1.0));
            sequence.play(path.position.x.to(20.0));
        });
        sequence.play(path.position.x.offset(20.0));
    });
    let mut rendered = vello::Scene::new();
    let size = Size::new(100.0, 100.0);
    for (time, expected_end, rebuilds) in [
        (0, None, 1),
        (500, Some(Point::new(10.0, 0.0)), 2),
        (1000, Some(Point::new(30.0, 0.0)), 3),
        (1500, Some(Point::new(30.0, 0.0)), 3),
        (750, Some(Point::new(20.0, 0.0)), 4),
        (0, None, 5),
    ] {
        animation.sample(ms(time));
        rendered.reset();
        animation.render(&mut rendered, size, size, 1.0);
        let geometry = &animation.path_geometry[path.id.0];
        let actual_end = geometry
            .visible
            .segments()
            .last()
            .map(|seg| seg.eval(1.0));
        assert_eq!(actual_end, expected_end);
        assert_eq!(geometry.rebuilds, rebuilds);
        assert_eq!(
            rendered.encoding().n_paths,
            u32::from(expected_end.is_some())
        );
    }
}

#[test]
fn path_cache_handles_bezier_geometry_and_style_changes() {
    let mut curve = BezPath::new();
    curve.move_to((0.0, 0.0));
    curve.quad_to((10.0, 20.0), (20.0, 0.0));
    let mut path = Path::from_bez_path(curve).end(0.5);
    let mut cache = PathGeometry::new(&path);
    assert_eq!(
        cache.visible.segments().last().unwrap().eval(1.0),
        Point::new(10.0, 10.0)
    );
    let mut rendered = vello::Scene::new();
    path.position = Point::new(20.0, 30.0);
    path.opacity = 0.5;
    path.stroke = colors::RED;
    path.stroke_width = 3.0;
    path.scale = 2.0;
    path.rotation = 0.4;
    cache.render(&path, &mut rendered, Affine::IDENTITY);
    assert_eq!(cache.rebuilds, 1);
    path.geometry = Path::new([(0.0, 0.0), (40.0, 0.0)]).geometry;
    rendered.reset();
    cache.render(&path, &mut rendered, Affine::IDENTITY);
    assert_eq!(
        cache.visible.segments().last().unwrap().eval(1.0),
        Point::new(20.0, 0.0)
    );
    assert_eq!(cache.rebuilds, 2);
    path.start = 0.75; // An empty/reversed visible range draws nothing.
    rendered.reset();
    cache.render(&path, &mut rendered, Affine::IDENTITY);
    assert_eq!(rendered.encoding().n_paths, 0);
}

#[test]
fn circle_ring_rendering_matches_parent_scale_and_derived_pulse_math()
{
    let mut scene = Scene::new();
    let color = colors::MAUVE;
    let dot = scene.add(
        Circle::new(18.0)
            .position((40.0, 60.0))
            .fill(color)
            .scale(0.0),
    );
    let ring = scene.add(
        Circle::new(18.0)
            .relative_to(dot)
            .fill(colors::TRANSPARENT)
            .stroke(color, 3.0),
    );
    let mut animation = scene.animate(|sequence| {
        sequence.defaults(s(1), ease::linear);
        sequence.play(
            dot.scale
                .to(1.0)
                .over(ms(400))
                .ease(ease::back::ease_out),
        );
        sequence.parallel(|sequence| {
            sequence.play(ring.scale.to(2.4));
            sequence.play(ring.opacity.to(0.0));
        });
    });
    let size = Size::new(200.0, 200.0);
    let shape = CircleShape::new(Point::ZERO, 18.0);
    for time in [0, 100, 200, 400, 900, 1400, 900, 0] {
        animation.sample(ms(time));
        let mut actual = vello::Scene::new();
        animation.render(&mut actual, size, size, 1.0);

        // Independent direct rendering of the original component equations.
        let scale =
            ease::back::ease_out((time as f32 / 400.0).min(1.0))
                as f64;
        let pulse = (time as f64 - 400.0).clamp(0.0, 1000.0) / 1000.0;
        let mut expected = vello::Scene::new();
        if scale > 0.0 {
            let xf = Affine::translate((100.0, 100.0))
                * Affine::translate((40.0, -60.0))
                * Affine::scale(scale);
            expected.fill(Fill::NonZero, xf, color, None, &shape);
            if pulse < 1.0 {
                expected.stroke(
                    &Stroke::new(3.0),
                    xf * Affine::scale(1.0 + pulse * 1.4),
                    color.with_alpha((1.0 - pulse) as f32),
                    None,
                    &shape,
                );
            }
        }
        assert_eq!(
            actual.encoding().draw_data,
            expected.encoding().draw_data,
            "at {time}ms"
        );
        assert_eq!(
            actual.encoding().path_data,
            expected.encoding().path_data,
            "at {time}ms"
        );
        assert_eq!(
            actual.encoding().transforms,
            expected.encoding().transforms,
            "at {time}ms"
        );
    }
}
