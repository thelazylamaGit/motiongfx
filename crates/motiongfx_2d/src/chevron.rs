//! [`Chevron`]: an open arrowhead outline.
use crate::rounded_triangle::Geometry;
use crate::{Color, DrawObject, Object, Point, SceneObject, World};
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
    /// The outline in local coordinates.
    pub fn geometry(&self) -> Geometry {
        Geometry::sharp_chevron(self.vertices())
    }
    /// How far the chevron reaches back from its apex along a line, so a
    /// line tip can trim the line to meet it.
    pub fn extent(&self) -> f64 {
        self.geometry().extent(
            self.position,
            self.rotation,
            self.scale,
        )
    }
}

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
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

crate::properties::handle_methods!(ChevronHandle,Chevron,ChevronId;
position: Point;
position_x: f64;
position_y: f64;
size: f64;
angle: f64;
rotation: f64;
scale: f64;
opacity: f64;
stroke_width: f64;
fill: Color;
stroke: Color;
line_cap: crate::Cap;
line_join: crate::Join;
);

crate::properties::transform_methods!(
    ChevronHandle,
    Chevron,
    ChevronId
);

crate::properties::fill_methods!(ChevronHandle, Chevron, ChevronId);
