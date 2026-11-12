//! Geometry/compositing regressions for decorated native lines.

use super::*;
use vello::kurbo::{Line as LineShape, PathEl};

#[test]
fn dashed_arrows_trim_offsets_and_reveal_in_both_coordinate_directions()
 {
    // Expected shaft/triangle coordinates are in Vello's y-down space.
    for (end, shaft, start_arrow, end_arrow) in [
        (
            (110.0, 20.0),
            [(21.0, -20.0), (52.5, -20.0)],
            [(15.0, -20.0), (27.0, -32.0), (27.0, -8.0)],
            [(58.5, -20.0), (46.5, -8.0), (46.5, -32.0)],
        ),
        (
            (10.0, 120.0),
            [(10.0, -31.0), (10.0, -62.5)],
            [(10.0, -25.0), (-2.0, -37.0), (22.0, -37.0)],
            [(10.0, -68.5), (22.0, -56.5), (-2.0, -56.5)],
        ),
    ] {
        let mut scene = Scene::new();
        scene.add(
            Line::new((10.0, 20.0), end)
                .stroke(colors::BLUE, 4.0)
                .line_dash([20.0, 20.0])
                .dash_offset(3.0)
                .start_arrow(true)
                .end_arrow(true)
                .arrow_size(12.0)
                .start_offset(5.0)
                .end_offset(8.0)
                .reveal(0.5)
                .opacity(0.5),
        );
        let mut animation =
            scene.animate(|sequence| sequence.wait(s(1)));
        let mut actual = vello::Scene::new();
        let size = Size::new(200.0, 200.0);
        animation.render(&mut actual, size, size, 1.0);
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
        for triangle in [start_arrow, end_arrow] {
            let geometry = [
                PathEl::MoveTo(triangle[0].into()),
                PathEl::LineTo(triangle[1].into()),
                PathEl::LineTo(triangle[2].into()),
                PathEl::ClosePath,
            ];
            expected.fill(Fill::NonZero, xf, color, None, &geometry);
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
fn degenerate_or_fully_trimmed_arrows_emit_no_geometry() {
    for (start, end, trim) in [
        ((0.0, 0.0), (0.0, 0.0), 0.0),
        ((0.0, 0.0), (10.0, 0.0), 20.0),
    ] {
        let mut scene = Scene::new();
        scene.add(
            Line::new(start, end)
                .start_arrow(true)
                .end_arrow(true)
                .end_offset(trim),
        );
        let mut animation =
            scene.animate(|sequence| sequence.wait(s(1)));
        let mut rendered = vello::Scene::new();
        let size = Size::new(100.0, 100.0);
        animation.render(&mut rendered, size, size, 1.0);
        assert_eq!(rendered.encoding().n_paths, 0);
    }
}
