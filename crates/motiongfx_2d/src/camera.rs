//! One scene-wide camera, sampled by the ordinary MotionGfx timeline.

use crate::{Graphics, Parent, Point};
use kurbo::Affine;
use motiongfx::prelude::SubjectSource;

/// A view in centered, y-up canvas coordinates. Frame scale 0.25 shows a
/// quarter of the normal width/height, making objects four times as large.
/// Rotation is the camera's counterclockwise angle in radians;
/// the rendered world moves in the opposite direction.
#[derive(Clone)]
pub struct Camera {
    pub position: Point,
    pub scale: f64,
    pub rotation: f64,
    target: Option<Parent>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Point::ZERO,
            scale: 1.0,
            rotation: 0.0,
            target: None,
        }
    }
}

impl Camera {
    pub fn new<M: crate::Model2d>() -> crate::Object<Self, M> {
        crate::Object::new(Self::default())
    }
}

crate::object::properties!(Camera; position: Point, scale: f64, rotation: f64);
impl<M: crate::Model2d> crate::Object<Camera, M> {
    /// Follow an already-created rectangle or circle in this scene. Only
    /// its world position is inherited, including its ancestors' transforms.
    /// The camera's position remains an offset in world axes; target rotation
    /// and scale do not rotate or zoom the camera.
    pub fn relative_to(mut self, target: impl Into<Parent>) -> Self {
        self.target = Some(target.into());
        self
    }
}

impl Camera {
    fn view_transform(&self, graphics: &Graphics) -> Affine {
        assert!(
            self.scale.is_finite() && self.scale > 0.0,
            "camera scale must be finite and greater than zero"
        );
        assert!(
            self.position.x.is_finite()
                && self.position.y.is_finite()
                && self.rotation.is_finite(),
            "camera position and rotation must be finite"
        );
        // Parent transforms operate in Vello's y-down coordinates. Extract
        // only their origin so following never inherits scale or rotation.
        let target =
            graphics.parent_transform(self.target) * Point::ZERO;
        let center = target + (self.position.x, -self.position.y);
        // Interpolate view size, then invert it to obtain magnification.
        // Interpolating magnification itself gives a different camera path.
        Affine::scale(self.scale.recip())
            * Affine::rotate(self.rotation)
            * Affine::translate((-center.x, -center.y))
    }
}

/// Identity of the scene's single camera.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct CameraId;

#[derive(Clone, Copy)]
pub struct CameraHandle {
    pub position: camera_position,
    pub scale: fields::scale::Target,
    pub rotation: fields::rotation::Target,
}

pub(crate) fn camera_handle() -> CameraHandle {
    CameraHandle {
        position: camera_position_axes(CameraId),
        scale: motiongfx_authoring::Target::new(CameraId),
        rotation: motiongfx_authoring::Target::new(CameraId),
    }
}
impl<M: crate::Model2d> crate::SceneObject<M> for Camera {
    type Handle = CameraHandle;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> CameraHandle {
        scene.model_mut().graphics_mut().camera = self;
        camera_handle()
    }
}

impl Graphics {
    pub fn camera_transform(&self) -> Affine {
        self.camera.view_transform(self)
    }
}

impl<M: crate::Model2d> SubjectSource<CameraId, Camera>
    for motiongfx_authoring::World<M>
{
    fn get_source(&self, _: CameraId) -> Option<&Camera> {
        Some(&self.model().graphics().camera)
    }
    fn apply_source<R>(
        &mut self,
        _: CameraId,
        f: impl FnOnce(&mut Camera) -> R,
    ) -> Option<R> {
        Some(f(&mut self.model_mut().graphics_mut().camera))
    }
}

// Fixed field identity is generated locally; no accessor metadata is stored in
// each handle. See properties.rs and motiongfx_authoring::Target.
crate::properties::fields!(fields, Camera, CameraId;
scale: f64 => new(Camera.scale);
rotation: f64 => new(Camera.rotation);
position: Point => new(Camera.position);
position_x: f64 => new(Camera.position.x);
position_y: f64 => new(Camera.position.y);
);
#[allow(non_camel_case_types)]
pub type camera_position = crate::Position<
    CameraId,
    Camera,
    fields::position::Target,
    fields::position_x::Target,
    fields::position_y::Target,
>;
pub fn camera_position_axes(id: CameraId) -> camera_position {
    crate::Position::from_targets(
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
        motiongfx_authoring::Target::new(id),
    )
}
