//! [`Chevron`]: an open arrowhead outline.
use crate::rounded_triangle::Geometry;
use crate::{
    Cap, Color, DrawObject, Handle, Join, Model2d, Object,
    SceneObject,
};
use kurbo::Affine;

/// An open arrowhead outline facing +x, with its apex at `position`: two
/// strokes meeting at the apex, so it has a stroke but no fill. `line_cap`
/// and `line_join` make its ends and apex flat or rounded. Create one with
/// [`Chevron::new`]; it also works as a line tip.
#[derive(Clone)]
#[non_exhaustive]
pub struct Chevron {
    /// Length from the apex to the base, in canvas units.
    pub size: f64,
    /// Full opening angle at the apex, in radians.
    pub angle: f64,
    /// Position, scale and rotation. The position is the where the apex is, in canvas units (y up).
    pub transform: crate::Transform,
    /// 0 (invisible) to 1 (opaque).
    pub opacity: f64,
    /// Outline colour.
    pub stroke: Color,
    /// Outline width in canvas units.
    pub stroke_width: f64,
    /// How the open ends are drawn.
    pub line_cap: Cap,
    /// How the apex corner is drawn.
    pub line_join: Join,
}

impl Default for Chevron {
    fn default() -> Self {
        Self {
            size: 24.0,
            angle: std::f64::consts::FRAC_PI_3,
            transform: crate::Transform::default(),
            opacity: 1.0,
            stroke: Color::WHITE,
            stroke_width: 2.0,
            line_cap: Cap::Round,
            line_join: Join::Round,
        }
    }
}

impl Chevron {
    /// A white chevron 24 units long with a 60° opening and a 2-unit
    /// stroke; set other fields with the builder methods.
    pub fn new<M: Model2d>() -> Object<Self, M> {
        Object::new(Self::default())
    }
    /// The outline in local coordinates.
    pub fn geometry(&self) -> Geometry {
        Geometry::sharp_chevron(crate::wedge::vertices(
            self.size, self.angle,
        ))
    }
    /// How far the chevron reaches back from its apex along a line, so a
    /// line tip can trim the line to meet it.
    pub fn extent(&self) -> f64 {
        self.geometry().extent(
            self.transform.position,
            self.transform.rotation,
            self.transform.scale,
        )
    }
    /// Where the chevron's local coordinates sit on the canvas.
    pub fn transform(&self) -> Affine {
        self.transform.affine()
    }
    /// Add the chevron and draw it in insertion order.
    pub(crate) fn insert<M: Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Handle<Self> {
        let handle = self.insert_tip(scene);
        scene
            .model_mut()
            .canvas_mut()
            .order
            .push(DrawObject::Chevron(handle.id));
        handle
    }
    /// Add the chevron without drawing it; its line draws it as a tip.
    pub(crate) fn insert_tip<M: Model2d>(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Handle<Self> {
        let chevrons = &mut scene.model_mut().canvas_mut().chevrons;
        chevrons.push(self);
        Handle::new(ChevronId(chevrons.len() - 1))
    }
}

/// Identifies one [`Chevron`] in its scene.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct ChevronId(pub(crate) usize);
impl ChevronId {
    /// Its position in the scene's storage for chevrons.
    pub fn index(self) -> usize {
        self.0
    }
}

impl<M: Model2d> SceneObject<M> for Chevron {
    type Handle = Handle<Self>;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Handle<Self> {
        self.insert(scene)
    }
}

crate::capability::stored_in!(Chevron, ChevronId, chevrons);
crate::object::stroke!(Chevron);
crate::object::stroke_options!(Chevron);

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
crate::properties::object!(Chevron, ChevronId;
transform(Chevron.transform);
set size: f64 => new(Chevron.size);
set angle: f64 => new(Chevron.angle);
set opacity: f64 => new(Chevron.opacity) shared;
get stroke: Color => new(Chevron.stroke) shared;
set stroke_width: f64 => new(Chevron.stroke_width) shared;
get line_cap: crate::Cap => discrete(Chevron.line_cap);
get line_join: crate::Join => discrete(Chevron.line_join);
);
crate::capability::capabilities!(Chevron; Transformable, Strokable, Fadable);
