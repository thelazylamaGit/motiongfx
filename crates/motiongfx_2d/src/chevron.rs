//! Open chevron state, typed targets and geometry.
use crate::rounded_triangle::Geometry;
use crate::{
    Color, DrawObject, Object, Point, Position, SceneObject, World,
};
use kurbo::Affine;
use motiongfx::prelude::SubjectSource;

crate::wedge::wedge!(
    Chevron,
    ChevronHandle,
    ChevronId,
    chevrons,
    chevron,
    false
);

impl Chevron {
    pub fn geometry(&self) -> Geometry {
        Geometry::sharp(self.vertices(), false)
    }
    pub fn extent(&self) -> f64 {
        self.geometry().extent(
            self.position,
            self.rotation,
            self.scale,
        )
    }
}

// Fixed field identity is generated locally; no accessor metadata is stored in
// each handle. See properties.rs and motiongfx_authoring::Target.
crate::properties::fields!(fields, Chevron, ChevronId;
position: Point => new(Chevron.position);
position_x: f64 => new(Chevron.position.x);
position_y: f64 => new(Chevron.position.y);
size: f64 => new(Chevron.size);
angle: f64 => new(Chevron.angle);
rotation: f64 => new(Chevron.rotation);
scale: f64 => new(Chevron.scale);
opacity: f64 => new(Chevron.opacity);
stroke_width: f64 => new(Chevron.stroke_width);
fill: Color => new(Chevron.fill);
stroke: Color => new(Chevron.stroke);
line_cap: crate::Cap => discrete(Chevron.line_cap);
line_join: crate::Join => discrete(Chevron.line_join);
);
