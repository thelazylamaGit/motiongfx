//! [`Chevron`]: an open arrowhead outline.
use crate::rounded_triangle::Geometry;
use crate::{Color, DrawObject, Object, Point, SceneObject};
use kurbo::Affine;

crate::wedge::wedge!(Chevron, ChevronId, chevrons, chevron, false);

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
crate::properties::object!(Chevron, ChevronId;
set size: f64 => new(Chevron.size);
set angle: f64 => new(Chevron.angle);
set position: Point => new(Chevron.position) shared;
set rotation: f64 => new(Chevron.rotation) shared;
set scale: f64 => new(Chevron.scale) shared;
set opacity: f64 => new(Chevron.opacity) shared;
set fill: Color => new(Chevron.fill) shared;
get stroke: Color => new(Chevron.stroke) shared;
set stroke_width: f64 => new(Chevron.stroke_width) shared;
get line_cap: crate::Cap => discrete(Chevron.line_cap);
get line_join: crate::Join => discrete(Chevron.line_join);
get position_x: f64 => new(Chevron.position.x) shared;
get position_y: f64 => new(Chevron.position.y) shared;
);
crate::capability::capabilities!(Chevron; Transformable, Fillable, Strokable, Fadable);
