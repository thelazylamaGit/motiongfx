//! 2D methods on the shared `Scene` and `CompiledScene` types, plus read
//! access to [`Graphics`] for renderers.
use crate::*;
use motiongfx_authoring::Property;
/// Camera and background setup for a 2D scene. In the prelude.
pub trait Scene2dExt<M: Model2d> {
    /// The scene's camera, to animate. Every scene has exactly one.
    fn camera(&self) -> CameraHandle;
    /// Set the camera's starting state, such as following an object.
    fn set_camera(
        &mut self,
        camera: impl SceneObject<M, Handle = CameraHandle>,
    ) -> CameraHandle;
    /// Set the background colour: fixed, or following a signal or computed
    /// value. The last call wins. Returns the background property, so a
    /// fixed colour can be animated with `.to(..)`.
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
/// Read 2D objects as last sampled, by handle. In the prelude.
///
/// # Panics
///
/// The object readers panic for a handle from another scene with fewer
/// objects; one from a scene with more reads the wrong object.
pub trait CompiledScene2dExt<M: Model2d> {
    /// The background colour.
    fn background(&self) -> Color;
    /// The camera.
    fn camera(&self) -> &Camera;
    /// A rectangle.
    fn rectangle(&self, handle: RectangleHandle) -> &Rectangle;
    /// A circle, including line tips.
    fn circle(&self, handle: CircleHandle) -> &Circle;
    /// A triangle, including line tips.
    fn triangle(&self, handle: TriangleHandle) -> &Triangle;
    /// A chevron, including line tips.
    fn chevron(&self, handle: ChevronHandle) -> &Chevron;
    /// A line, from a plain or tipped line handle.
    fn line(&self, handle: impl Into<LineHandle>) -> &Line;
    /// A path.
    fn path(&self, handle: PathHandle) -> &Path;
    /// A text object.
    fn text(&self, handle: TextHandle) -> &Text;
}
impl<M: Model2d> CompiledScene2dExt<M>
    for motiongfx_authoring::CompiledScene<M>
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
/// Read access for renderers. Each list is indexed by its object IDs'
/// `index()`.
impl Graphics {
    /// The background colour.
    pub fn background(&self) -> Color {
        self.background.0
    }
    /// The camera.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }
    /// Every rectangle.
    pub fn rectangles(&self) -> &[Rectangle] {
        &self.rectangles
    }
    /// Every circle, including line tips.
    pub fn circles(&self) -> &[Circle] {
        &self.circles
    }
    /// Every triangle, including line tips.
    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }
    /// Every chevron, including line tips.
    pub fn chevrons(&self) -> &[Chevron] {
        &self.chevrons
    }
    /// Every line.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
    /// Every path.
    pub fn paths(&self) -> &[Path] {
        &self.paths
    }
    /// Every text object.
    pub fn texts(&self) -> &[Text] {
        &self.texts
    }
    /// Every text selection that some animation used.
    pub fn text_selections(&self) -> &[TextSelection] {
        &self.text_selections
    }
    /// The objects to draw, back to front. Line tips are drawn with their
    /// line, so they aren't listed.
    pub fn draw_order(&self) -> &[DrawObject] {
        &self.order
    }
    /// The transform `text` inherits from its parent (identity if none).
    pub fn text_parent_transform(&self, text: &Text) -> Affine {
        self.parent_transform(text.parent)
    }
    /// The transform `circle` inherits from its parent (identity if none).
    pub fn circle_parent_transform(&self, circle: &Circle) -> Affine {
        self.parent_transform(circle.parent)
    }
}
impl Rectangle {
    /// Where the rectangle's local coordinates sit on the canvas. Scale is
    /// applied to the size instead, so strokes keep their width.
    pub fn transform(&self) -> Affine {
        Affine::translate((self.position.x, -self.position.y))
            * Affine::rotate(-self.rotation)
    }
}
