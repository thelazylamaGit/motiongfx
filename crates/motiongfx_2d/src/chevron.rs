//! [`Chevron`]: an open arrowhead outline.
use crate::rounded_triangle::Geometry;
use crate::{Color, DrawObject, Object, Point, SceneObject};
use kurbo::Affine;

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
crate::properties::object!(Chevron, ChevronHandle, ChevronId;
set size: f64 => new(Chevron.size);
set angle: f64 => new(Chevron.angle);
set position: Point => new(Chevron.position);
set rotation: f64 => new(Chevron.rotation);
set scale: f64 => new(Chevron.scale);
set opacity: f64 => new(Chevron.opacity);
set fill: Color => new(Chevron.fill);
get stroke: Color => new(Chevron.stroke);
set stroke_width: f64 => new(Chevron.stroke_width);
get line_cap: crate::Cap => discrete(Chevron.line_cap);
get line_join: crate::Join => discrete(Chevron.line_join);
get position_x: f64 => new(Chevron.position.x);
get position_y: f64 => new(Chevron.position.y);
);
crate::capability::handle_traits!(ChevronHandle => Chevron, ChevronId;
    Transformable, Fillable, Strokable, Fadable);
