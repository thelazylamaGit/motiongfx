//! Filled triangle state, typed targets and rounded geometry.
use crate::rounded_triangle::{self, Geometry};
use crate::{
    Color, DrawObject, Object, Point, Position, SceneObject, World,
};
use kurbo::Affine;
use motiongfx::prelude::SubjectSource;

crate::wedge::wedge!(Triangle, TriangleHandle, TriangleId, triangles, triangle, true, radius => triangle_geometry);

impl Triangle {
    pub fn geometry(&self) -> Geometry {
        rounded_triangle::rounded(self.vertices(), self.radius)
    }
}

// Fixed field identity is generated locally; no accessor metadata is stored in
// each handle. See properties.rs and motiongfx_authoring::Target.
crate::properties::fields!(fields, Triangle, TriangleId;
position: Point => new(Triangle.position);
position_x: f64 => new(Triangle.position.x);
position_y: f64 => new(Triangle.position.y);
size: f64 => new(Triangle.size);
angle: f64 => new(Triangle.angle);
radius: f64 => new(Triangle.radius);
rotation: f64 => new(Triangle.rotation);
scale: f64 => new(Triangle.scale);
opacity: f64 => new(Triangle.opacity);
stroke_width: f64 => new(Triangle.stroke_width);
fill: Color => new(Triangle.fill);
stroke: Color => new(Triangle.stroke);
line_cap: crate::Cap => discrete(Triangle.line_cap);
line_join: crate::Join => discrete(Triangle.line_join);
);
