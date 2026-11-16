//! [`Triangle`]: a filled arrowhead, optionally with rounded corners.
use crate::rounded_triangle::{self, Geometry};
use crate::{Color, DrawObject, Object, Point, SceneObject};
use kurbo::Affine;

crate::wedge::wedge!(Triangle, TriangleId, triangles, triangle, true, radius => triangle_geometry);

impl Triangle {
    /// The outline in local coordinates, with rounded corners if `radius`
    /// is set.
    pub fn geometry(&self) -> Geometry {
        rounded_triangle::rounded(self.vertices(), self.radius)
    }
}

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
crate::properties::object!(Triangle, TriangleId;
set size: f64 => new(Triangle.size);
set angle: f64 => new(Triangle.angle);
set radius: f64 => new(Triangle.radius);
set position: Point => new(Triangle.position) shared;
set rotation: f64 => new(Triangle.rotation) shared;
set scale: f64 => new(Triangle.scale) shared;
set opacity: f64 => new(Triangle.opacity) shared;
set fill: Color => new(Triangle.fill) shared;
get stroke: Color => new(Triangle.stroke) shared;
set stroke_width: f64 => new(Triangle.stroke_width) shared;
get line_cap: crate::Cap => discrete(Triangle.line_cap);
get line_join: crate::Join => discrete(Triangle.line_join);
get position_x: f64 => new(Triangle.position.x) shared;
get position_y: f64 => new(Triangle.position.y) shared;
);
crate::capability::capabilities!(Triangle; Transformable, Fillable, Strokable, Fadable);
