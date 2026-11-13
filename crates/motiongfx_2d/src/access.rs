//! 2D conveniences on the shared scene lifecycle.
use crate::*;
use motiongfx_authoring::Property;
/// 2D configuration on the shared authoring scene. Import this trait (or the
/// prelude) to keep the shared lifecycle independent of camera/background types.
pub trait Scene2dExt<M: Model2d> {
    /// Handles for the single existing camera; repeated calls return the same targets.
    fn camera(&self) -> CameraHandle;
    /// Replace initial camera state; this does not add a second camera.
    fn set_camera(
        &mut self,
        camera: impl SceneObject<M, Handle = CameraHandle>,
    ) -> CameraHandle;
    /// Fixed color or reactive clear/base color. The last call wins. For a
    /// fixed color the returned property can be animated with ordinary actions.
    fn set_background(
        &mut self,
        color: impl Into<Input<Color, M>>,
    ) -> Property<BackgroundId, Color, Color>;
}
impl<M: Model2d> Scene2dExt<M> for motiongfx_authoring::Scene<M> {
    fn camera(&self) -> CameraHandle {
        camera::camera_handle()
    }
    fn set_camera(
        &mut self,
        camera: impl SceneObject<M, Handle = CameraHandle>,
    ) -> CameraHandle {
        self.add(camera)
    }
    fn set_background(
        &mut self,
        color: impl Into<Input<Color, M>>,
    ) -> Property<BackgroundId, Color, Color> {
        let color = color.into();
        if let Some(value) = color.as_fixed() {
            self.model_mut().graphics_mut().background.0 = *value;
        }
        self.prepare_with::<BackgroundId>(move |scene| {
            if color.as_fixed().is_none() {
                scene.bind(
                    background::property(),
                    color,
                    std::convert::identity,
                );
            }
        });
        background::property()
    }
}
/// Read sampled 2D objects by typed handle. Handles must belong to this scene;
/// the pre-existing ID representation does not enforce scene identity.
pub trait Animation2dExt<M: Model2d> {
    fn background(&self) -> Color;
    fn camera(&self) -> &Camera;
    fn rectangle(&self, handle: RectangleHandle) -> &Rectangle;
    fn circle(&self, handle: CircleHandle) -> &Circle;
    fn triangle(&self, handle: TriangleHandle) -> &Triangle;
    fn chevron(&self, handle: ChevronHandle) -> &Chevron;
    fn line(&self, handle: impl Into<LineHandle>) -> &Line;
    fn path(&self, handle: PathHandle) -> &Path;
    fn text(&self, handle: TextHandle) -> &Text;
}
impl<M: Model2d> Animation2dExt<M>
    for motiongfx_authoring::Animation<M>
{
    fn background(&self) -> Color {
        self.model().graphics().background.0
    }
    fn camera(&self) -> &Camera {
        &self.model().graphics().camera
    }
    fn rectangle(&self, h: RectangleHandle) -> &Rectangle {
        &self.model().graphics().rectangles[h.id.0]
    }
    fn circle(&self, h: CircleHandle) -> &Circle {
        &self.model().graphics().circles[h.id.0]
    }
    fn triangle(&self, h: TriangleHandle) -> &Triangle {
        &self.model().graphics().triangles[h.id.0]
    }
    fn chevron(&self, h: ChevronHandle) -> &Chevron {
        &self.model().graphics().chevrons[h.id.0]
    }
    fn line(&self, h: impl Into<LineHandle>) -> &Line {
        &self.model().graphics().lines[h.into().id.0]
    }
    fn path(&self, h: PathHandle) -> &Path {
        &self.model().graphics().paths[h.id.0]
    }
    fn text(&self, h: TextHandle) -> &Text {
        &self.model().graphics().texts[h.id.0]
    }
}
impl Graphics {
    pub fn background(&self) -> Color {
        self.background.0
    }
    pub fn camera(&self) -> &Camera {
        &self.camera
    }
    pub fn rectangles(&self) -> &[Rectangle] {
        &self.rectangles
    }
    pub fn circles(&self) -> &[Circle] {
        &self.circles
    }
    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }
    pub fn chevrons(&self) -> &[Chevron] {
        &self.chevrons
    }
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
    pub fn paths(&self) -> &[Path] {
        &self.paths
    }
    pub fn texts(&self) -> &[Text] {
        &self.texts
    }
    pub fn text_selections(&self) -> &[TextSelection] {
        &self.text_selections
    }
    pub fn draw_order(&self) -> &[DrawObject] {
        &self.order
    }
    pub fn text_parent_transform(&self, text: &Text) -> Affine {
        self.parent_transform(text.parent)
    }
    pub fn circle_parent_transform(&self, circle: &Circle) -> Affine {
        self.parent_transform(circle.parent)
    }
}
impl Rectangle {
    pub fn transform(&self) -> Affine {
        Affine::translate((self.position.x, -self.position.y))
            * Affine::rotate(-self.rotation)
    }
}
