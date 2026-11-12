//! Circular corner fillets for closed triangles. Coordinates are y-up.
//! Fixed-capacity paths avoid allocating a buffer while constructing geometry.

use crate::Point;
use kurbo::{Affine, ParamCurveExtrema, PathEl, segments};

#[derive(Clone, Copy)]
pub struct Geometry {
    elements: [PathEl; 11],
    len: usize,
}
impl Geometry {
    fn new() -> Self {
        Self {
            elements: [PathEl::ClosePath; 11],
            len: 0,
        }
    }
    fn push(&mut self, element: PathEl) {
        self.elements[self.len] = element;
        self.len += 1;
    }
    pub fn sharp([tip, a, b]: [Point; 3], closed: bool) -> Self {
        let mut path = Self::new();
        path.push(PathEl::MoveTo(a));
        path.push(PathEl::LineTo(tip));
        path.push(PathEl::LineTo(b));
        if closed {
            path.push(PathEl::ClosePath);
        }
        path
    }
    pub fn elements(&self) -> &[PathEl] {
        &self.elements[..self.len]
    }
    pub fn canvas(mut self) -> Self {
        for element in &mut self.elements[..self.len] {
            let flip = |p: Point| Point::new(p.x, -p.y);
            *element = match *element {
                PathEl::MoveTo(p) => PathEl::MoveTo(flip(p)),
                PathEl::LineTo(p) => PathEl::LineTo(flip(p)),
                PathEl::QuadTo(a, b) => {
                    PathEl::QuadTo(flip(a), flip(b))
                }
                PathEl::CurveTo(a, b, c) => {
                    PathEl::CurveTo(flip(a), flip(b), flip(c))
                }
                PathEl::ClosePath => PathEl::ClosePath,
            };
        }
        self
    }
    pub fn extent(
        &self,
        position: Point,
        rotation: f64,
        scale: f64,
    ) -> f64 {
        let transform = Affine::translate((position.x, position.y))
            * Affine::rotate(rotation)
            * Affine::scale(scale);
        let min_x = segments(
            self.elements().iter().map(|el| transform * *el),
        )
        .map(|segment| segment.bounding_box().x0)
        .fold(0.0, f64::min);
        -min_x
    }
}

/// Requested radius is clamped to the triangle's inradius, so all three
/// fillets fit without crossing. At the limit the boundary is the incircle.
pub fn rounded(vertices: [Point; 3], radius: f64) -> Geometry {
    assert!(radius.is_finite(), "triangle radius must be finite");
    let [tip, a, b] = vertices;
    let side_a = tip.distance(a);
    let side_b = a.distance(b);
    let side_c = b.distance(tip);
    let twice_area = (a - tip).cross(b - tip).abs();
    let perimeter = side_a + side_b + side_c;
    if radius <= 0.0 || twice_area == 0.0 || perimeter == 0.0 {
        return Geometry::sharp(vertices, true);
    }
    let radius = radius.min(twice_area / perimeter);
    // Match the sharp outline's winding and starting corner.
    let vertices = [a, tip, b];
    let corners = std::array::from_fn::<_, 3, _>(|i| {
        let vertex = vertices[i];
        let incoming = (vertex - vertices[(i + 2) % 3]).normalize();
        let outgoing = (vertices[(i + 1) % 3] - vertex).normalize();
        let turn =
            incoming.cross(outgoing).atan2(incoming.dot(outgoing));
        let tangent_distance = radius * (turn.abs() / 2.0).tan();
        let start = vertex - incoming * tangent_distance;
        let end = vertex + outgoing * tangent_distance;
        let normal = kurbo::Vec2::new(-incoming.y, incoming.x);
        let center = start + normal * (radius * turn.signum());
        (start, end, center, turn)
    });
    let mut path = Geometry::new();
    path.push(PathEl::MoveTo(corners[0].0));
    for (i, &(start, end, center, turn)) in corners.iter().enumerate()
    {
        if i > 0 {
            path.push(PathEl::LineTo(start));
        }
        // Each turn is less than pi: at most two <=90-degree cubic arcs.
        // This is the usual cubic approximation of a circular corner, also
        // used for vector circles/rounded rectangles.
        let count = (turn.abs() / std::f64::consts::FRAC_PI_2).ceil()
            as usize;
        let step = turn / count as f64;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let angle = (start.y - center.y).atan2(start.x - center.x);
        let mut from = start;
        for part in 0..count {
            let (s0, c0) = (angle + step * part as f64).sin_cos();
            let (s1, c1) =
                (angle + step * (part + 1) as f64).sin_cos();
            let to = if part + 1 == count {
                end
            } else {
                center + (radius * c1, radius * s1)
            };
            let control_a =
                from + (-s0 * radius * k, c0 * radius * k);
            let control_b = to
                - kurbo::Vec2::new(-s1 * radius * k, c1 * radius * k);
            path.push(PathEl::CurveTo(control_a, control_b, to));
            from = to;
        }
    }
    path.push(PathEl::LineTo(corners[0].0));
    path.push(PathEl::ClosePath);
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{ParamCurve, Shape};

    #[test]
    fn fillets_are_tangent_closed_and_inside_the_original_triangle() {
        let vertices = [
            Point::ZERO,
            Point::new(-100.0, 100.0),
            Point::new(-100.0, -100.0),
        ];
        let geometry = rounded(vertices, 10.0);
        assert!(geometry.elements().len() <= 11);
        assert!(matches!(
            geometry.elements().last(),
            Some(PathEl::ClosePath)
        ));
        let mut point = match geometry.elements()[0] {
            PathEl::MoveTo(p) => p,
            _ => unreachable!(),
        };
        let mut previous_tangent: Option<kurbo::Vec2> = None;
        for element in &geometry.elements()[1..] {
            let (incoming, outgoing, end) = match *element {
                PathEl::LineTo(end) => {
                    (end - point, end - point, end)
                }
                PathEl::CurveTo(a, b, end) => {
                    (a - point, end - b, end)
                }
                PathEl::ClosePath => continue,
                _ => unreachable!(),
            };
            if let Some(previous) = previous_tangent {
                assert!(
                    previous.normalize().dot(incoming.normalize())
                        > 1.0 - 1e-10
                );
            }
            previous_tangent = Some(outgoing);
            point = end;
        }
        for segment in segments(geometry.elements().iter().copied()) {
            for step in 0..=20 {
                let p = segment.eval(step as f64 / 20.0);
                assert!(
                    p.x >= -100.0 - 1e-8 && p.y.abs() <= -p.x + 1e-8
                );
            }
        }
        let original = Geometry::sharp(vertices, true);
        assert!(
            geometry.elements().area().abs()
                < original.elements().area().abs()
        );
    }

    #[test]
    fn zero_degenerate_and_oversized_radii_are_safe_and_clamped_to_the_incircle()
     {
        let vertices = [
            Point::ZERO,
            Point::new(-100.0, 100.0),
            Point::new(-100.0, -100.0),
        ];
        assert_eq!(
            rounded(vertices, 0.0).elements(),
            Geometry::sharp(vertices, true).elements()
        );
        assert_eq!(
            rounded(vertices, -2.0).elements(),
            rounded(vertices, 0.0).elements()
        );
        let radius = 100.0 / (std::f64::consts::SQRT_2 + 1.0);
        let clamped = rounded(vertices, 1e6);
        let center = Point::new(-100.0 + radius, 0.0);
        for segment in segments(clamped.elements().iter().copied()) {
            for step in 0..=20 {
                let distance =
                    segment.eval(step as f64 / 20.0).distance(center);
                assert!((distance - radius).abs() < radius * 3e-4);
            }
        }
        let area = clamped.elements().area().abs();
        assert!(
            (area - std::f64::consts::PI * radius * radius).abs()
                < area * 1e-3
        );
        let zero = rounded([Point::ZERO; 3], 100.0);
        assert_eq!(
            zero.elements(),
            Geometry::sharp([Point::ZERO; 3], true).elements()
        );
    }
}
