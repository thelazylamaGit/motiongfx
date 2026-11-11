//! One scene-wide camera, sampled by the ordinary MotionGfx timeline.

use crate::{Graphics, Parent, Point, Position, Scene};
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
use vello::kurbo::Affine;

/// A view in centered, y-up canvas coordinates. Zoom 2 makes objects twice
/// as large. Rotation is the camera's counterclockwise angle in radians;
/// the rendered world moves in the opposite direction.
#[derive(Clone)]
pub struct Camera {
    pub position: Point,
    pub zoom: f64,
    pub rotation: f64,
    target: Option<Parent>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Point::ZERO,
            zoom: 1.0,
            rotation: 0.0,
            target: None,
        }
    }
}

impl Camera {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn position(mut self, position: impl Into<Point>) -> Self {
        self.position = position.into();
        self
    }
    pub fn zoom(mut self, zoom: f64) -> Self {
        self.zoom = zoom;
        self
    }
    pub fn rotation(mut self, rotation: f64) -> Self {
        self.rotation = rotation;
        self
    }
    /// Follow an already-created rectangle or circle in this scene. Only
    /// its world position is inherited, including its ancestors' transforms.
    /// The camera's position remains an offset in world axes; target rotation
    /// and scale do not rotate or zoom the camera.
    pub fn relative_to(mut self, target: impl Into<Parent>) -> Self {
        self.target = Some(target.into());
        self
    }

    fn view_transform(&self, graphics: &Graphics) -> Affine {
        assert!(
            self.zoom.is_finite() && self.zoom > 0.0,
            "camera zoom must be finite and greater than zero"
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
        Affine::scale(self.zoom)
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
    pub position: Position<CameraId, Camera>,
    pub zoom: Property<CameraId, Camera, f64>,
    pub rotation: Property<CameraId, Camera, f64>,
}

impl Scene {
    /// Get animation handles for the existing camera. Every scene starts
    /// with an identity camera; repeated calls return the same targets.
    pub fn camera(&self) -> CameraHandle {
        CameraHandle {
            position: Position::new(
                CameraId,
                path!(Camera.position),
                path!(Camera.position.x),
                path!(Camera.position.y),
            ),
            zoom: Property::new(CameraId, path!(Camera.zoom)),
            rotation: Property::new(CameraId, path!(Camera.rotation)),
        }
    }

    /// Configure initial camera state, optionally attached to an object.
    /// This replaces the scene's single camera rather than adding another.
    pub fn set_camera(&mut self, camera: Camera) -> CameraHandle {
        self.graphics.camera = camera;
        self.camera()
    }
}

impl Graphics {
    pub(crate) fn camera_transform(&self) -> Affine {
        self.camera.view_transform(self)
    }
}

impl SubjectSource<CameraId, Camera> for Graphics {
    fn get_source(&self, _: CameraId) -> Option<&Camera> {
        Some(&self.camera)
    }
    fn apply_source<R>(
        &mut self,
        _: CameraId,
        f: impl FnOnce(&mut Camera) -> R,
    ) -> Option<R> {
        Some(f(&mut self.camera))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circle, Rectangle};
    use motiongfx::prelude::{ease, ms, s};

    fn near(actual: Point, expected: Point) {
        assert!(actual.distance(expected) < 1e-9);
    }

    #[test]
    fn camera_pan_zoom_rotation_render_and_reverse_seek() {
        let mut scene = Scene::new();
        let dot = scene.add(Circle::new(10.0));
        let camera = scene.camera();
        let mut animation = scene.animate(|sequence| {
            sequence.defaults(s(1), ease::linear);
            sequence.parallel(|sequence| {
                sequence.play(camera.position.to((100.0, 50.0)));
                sequence.play(camera.zoom.to(3.0));
                sequence.play(
                    camera.rotation.to(std::f64::consts::FRAC_PI_2),
                );
            });
            sequence.wait(s(1));
        });
        for time in [1000, 500, 2000, 0, 750] {
            animation.sample(ms(time));
            let t = (time as f64 / 1000.0).min(1.0);
            let view = animation.graphics.camera_transform();
            let center = Point::new(100.0 * t, -50.0 * t);
            near(view * center, Point::ZERO);
            let angle = t * std::f64::consts::FRAC_PI_2;
            let zoom = 1.0 + 2.0 * t;
            near(
                view * (center + (1.0, 0.0)),
                Point::new(zoom * angle.cos(), zoom * angle.sin()),
            );

            // Compare actual submission to a directly transformed circle,
            // including the existing canvas fitting and DPI transforms.
            let viewport = crate::Size::new(400.0, 300.0);
            let canvas = crate::Size::new(200.0, 200.0);
            let mut actual = vello::Scene::new();
            animation.render(&mut actual, viewport, canvas, 2.0);
            let mut expected = vello::Scene::new();
            animation.circle(dot).render(
                &mut expected,
                Affine::scale(2.0)
                    * Affine::translate((200.0, 150.0))
                    * Affine::scale(1.5)
                    * view,
            );
            assert_eq!(
                actual.encoding().transforms,
                expected.encoding().transforms
            );
            assert_eq!(
                actual.encoding().path_data,
                expected.encoding().path_data
            );
        }
    }

    #[test]
    fn follow_uses_nested_world_position_but_not_target_scale_or_rotation()
     {
        let mut scene = Scene::new();
        let parent = scene.add(Rectangle {
            position: Point::new(100.0, 50.0),
            scale: 2.0,
            rotation: std::f64::consts::FRAC_PI_2,
            ..Rectangle::default()
        });
        let target = scene.add(
            Circle::new(10.0)
                .relative_to(parent)
                .position((10.0, 0.0))
                .scale(3.0)
                .rotation(0.8),
        );
        let camera = scene.set_camera(
            Camera::new().relative_to(target).position((5.0, 6.0)),
        );
        let mut animation = scene.animate(|sequence| {
            sequence.defaults(s(1), ease::linear);
            sequence.parallel(|sequence| {
                sequence.play(parent.position.x.offset(100.0));
                sequence.play(target.position.x.to(20.0));
                sequence.play(target.scale.to(0.0));
                sequence.play(target.rotation.to(2.0));
                sequence.play(camera.position.x.to(15.0));
            });
        });
        for time in [0, 1000, 500, 0] {
            animation.sample(ms(time));
            let t = time as f64 / 1000.0;
            let center =
                Point::new(105.0 + 110.0 * t, -(76.0 + 20.0 * t));
            let view = animation.graphics.camera_transform();
            near(view * center, Point::ZERO);
            near(view * (center + (1.0, 0.0)), Point::new(1.0, 0.0));
        }
    }

    #[test]
    #[should_panic(
        expected = "camera zoom must be finite and greater than zero"
    )]
    fn invalid_zoom_is_rejected_before_rendering() {
        let mut scene = Scene::new();
        scene.set_camera(Camera::new().zoom(0.0));
        scene.graphics.camera_transform();
    }
}
