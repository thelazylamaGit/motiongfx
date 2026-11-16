//! [`Camera`]: the scene's view, which can pan, zoom, turn and follow.

use crate::{Graphics, Parent, Point};
use kurbo::Affine;

/// The scene's view. Every scene has one; get it with `scene.camera()`, or
/// set its starting state with `scene.set_camera(Camera::new()..)`.
#[derive(Clone)]
pub struct Camera {
    /// Point at the centre of the view, in canvas units (y up).
    pub position: Point,
    /// Size of the view frame: 0.5 shows half the width and height, so objects look twice as big.
    pub scale: f64,
    /// Rotation of the view in radians, counterclockwise; the scene turns the other way.
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
    /// A camera at the origin, unscaled and unrotated.
    pub fn new<M: crate::Model2d>() -> crate::Object<Self, M> {
        crate::Object::new(Self::default())
    }
}

impl<M: crate::Model2d> crate::Object<Camera, M> {
    /// Follow a rectangle or circle already in the scene: `position`
    /// becomes an offset from it. The camera follows its position only, not
    /// its rotation or scale.
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

/// Identifies the scene's camera.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct CameraId;

/// The scene's camera, from `scene.camera()`. Its methods return fields to
/// animate.
#[derive(Clone, Copy, Debug)]
pub struct CameraHandle {
    pub(crate) id: CameraId,
}

pub(crate) fn camera_handle() -> CameraHandle {
    CameraHandle { id: CameraId }
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
    /// The transform from canvas to view coordinates for the camera as last
    /// sampled, including any object it follows.
    pub fn camera_transform(&self) -> Affine {
        self.camera.view_transform(self)
    }
}

impl<M: crate::Model2d> motiongfx_authoring::WorldObject<CameraId, M>
    for Camera
{
    fn get(world: &crate::World<M>, _: CameraId) -> Option<&Self> {
        Some(&world.model().graphics().camera)
    }
    fn get_mut(
        world: &mut crate::World<M>,
        _: CameraId,
    ) -> Option<&mut Self> {
        Some(&mut world.model_mut().graphics_mut().camera)
    }
}

// Animatable fields: `new` ones interpolate, `discrete` ones jump.
crate::properties::object!(Camera, CameraHandle, CameraId;
set position: Point => new(Camera.position);
set scale: f64 => new(Camera.scale);
set rotation: f64 => new(Camera.rotation);
get position_x: f64 => new(Camera.position.x);
get position_y: f64 => new(Camera.position.y);
);
crate::capability::handle_traits!(CameraHandle => Camera, CameraId; Transformable);
