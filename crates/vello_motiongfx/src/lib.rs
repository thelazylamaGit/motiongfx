//! Minimal native graphics integration. Object state is stored once; the
//! authoring handles construct MotionGfx actions and typed native bindings.

use motiongfx::prelude::*;
use motiongfx_authoring::BindingPlan;
pub use motiongfx_authoring::{BindingError, BindingSource, Derived};
use motiongfx_authoring::{Property, Sequence};
pub use motiongfx_authoring::{animate, smooth};
use peniko_motiongfx as _;
use std::time::Duration;
use vello::kurbo::{Affine, Rect, Stroke};
pub use vello::kurbo::{Point, Size, Vec2};
pub use vello::peniko::Color;
use vello::peniko::Fill;

mod camera;
mod circle;
pub mod colors;
pub mod lettering;
mod line;
mod parent;
mod path;
mod path_renderer;
mod properties;
mod text;
mod text_renderer;
mod updates;
mod values;
pub use camera::{Camera, CameraHandle, CameraId};
pub use circle::{Circle, CircleHandle, CircleId};
pub use line::{Line, LineHandle, LineId};
pub use parent::Parent;
pub use path::{BezPath, Path, PathHandle, PathId};
use path_renderer::PathGeometry;
pub use updates::UpdateTargets;
pub use values::{SampleTime, ValueId};

#[cfg(test)]
mod binding_tests;
#[cfg(test)]
mod graphics_tests;
#[cfg(test)]
mod update_tests;
pub use properties::Position;
pub use text::{Text, TextAlign, TextHandle, TextId};
use text_renderer::TextLayout;

/// Rectangle centered on its position. Coordinates are logical canvas units,
/// positive y points up, and rotation is counterclockwise in radians.
/// Scale changes geometry without changing the stroke width.
#[derive(Clone)]
pub struct Rectangle {
    pub size: Size,
    pub position: Point,
    pub fill: Color,
    pub stroke: Color,
    pub stroke_width: f64,
    pub scale: f64,
    pub rotation: f64,
    pub radius: f64,
}

impl Default for Rectangle {
    fn default() -> Self {
        Self {
            size: Size::new(100.0, 100.0),
            position: Point::ZERO,
            fill: Color::WHITE,
            stroke: Color::TRANSPARENT,
            stroke_width: 0.0,
            scale: 1.0,
            rotation: 0.0,
            radius: 0.0,
        }
    }
}

impl Rectangle {
    /// Create initial rectangle state with the remaining properties defaulted.
    /// Register this value with `Scene::add` to obtain animation handles.
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            size: Size::new(width, height),
            ..Self::default()
        }
    }

    /// Create a rectangle from a `(width, height)` tuple, `Vec2`, or `Size`.
    pub fn from_size(size: impl Into<(f64, f64)>) -> Self {
        let (width, height) = size.into();
        Self::new(width, height)
    }

    pub fn fill(mut self, color: Color) -> Self {
        self.fill = color;
        self
    }

    pub fn position(mut self, position: impl Into<Point>) -> Self {
        self.position = position.into();
        self
    }

    /// Corner radius in unscaled canvas units, clamped to the shape by Kurbo.
    pub fn radius(mut self, radius: f64) -> Self {
        self.radius = radius;
        self
    }

    pub fn stroke(mut self, color: Color, width: f64) -> Self {
        self.stroke = color;
        self.stroke_width = width;
        self
    }
}

/// Typed subject identity; separate from authoring handles and sampled state.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct RectangleId(usize);

/// Setup-time property access, rather than the current property values.
#[derive(Clone, Copy)]
pub struct RectangleHandle {
    id: RectangleId,
    pub position: Position<RectangleId, Rectangle>,
    pub fill: Property<RectangleId, Rectangle, Color>,
    pub scale: Property<RectangleId, Rectangle, f64>,
    pub rotation: Property<RectangleId, Rectangle, f64>,
}

#[derive(Default)]
pub struct Graphics {
    values: Vec<Box<dyn std::any::Any + Send + Sync>>,
    sample_time: Duration,
    camera: Camera,
    rectangles: Vec<Rectangle>,
    circles: Vec<Circle>,
    lines: Vec<Line>,
    paths: Vec<Path>,
    texts: Vec<Text>,
    order: Vec<DrawObject>,
}

enum DrawObject {
    Rectangle(RectangleId),
    Circle(CircleId),
    Line(LineId),
    Path(PathId),
    Text(TextId),
}

impl Graphics {
    fn text_parent_transform(&self, text: &Text) -> Affine {
        self.parent_transform(text.parent)
    }
}

impl SubjectSource<RectangleId, Rectangle> for Graphics {
    fn get_source(&self, id: RectangleId) -> Option<&Rectangle> {
        self.rectangles.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: RectangleId,
        f: impl FnOnce(&mut Rectangle) -> R,
    ) -> Option<R> {
        self.rectangles.get_mut(id.0).map(f)
    }
}

impl SubjectSource<TextId, Text> for Graphics {
    fn get_source(&self, id: TextId) -> Option<&Text> {
        self.texts.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: TextId,
        f: impl FnOnce(&mut Text) -> R,
    ) -> Option<R> {
        self.texts.get_mut(id.0).map(f)
    }
}

/// Scene construction: owns initial graphics state in insertion/draw order.
/// `add` registers an object and returns its typed animation handles. `animate`
/// consumes this builder to produce the scene state plus a compiled timeline.
#[derive(Default)]
pub struct Scene {
    graphics: Graphics,
    bindings: BindingPlan<Graphics>,
    updates: Vec<updates::NativeUpdate>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate a property from explicit sources after timeline sampling.
    /// Sources may be properties, shared derived values, or tuples of either.
    /// Values use the object's local coordinates; parent transforms are applied
    /// later during rendering. Functions must depend only on declared inputs.
    ///
    /// Incorrect output types fail to compile:
    /// ```compile_fail
    /// use vello_motiongfx::{Circle, Scene};
    /// let mut scene = Scene::new();
    /// let dot = scene.add(Circle::new(10.0));
    /// // Position needs Point, but this calculation returns f64.
    /// scene.bind(dot.position, dot.radius, |radius| radius);
    /// ```
    pub fn bind<I, S, T, Source, F>(
        &mut self,
        destination: impl Into<Property<I, S, T>>,
        source: Source,
        calculate: F,
    ) where
        I: motiongfx::subject::SubjectId,
        S: 'static,
        T: Clone + ThreadSafe,
        Graphics: SubjectSource<I, S>,
        Source: BindingSource<Graphics>,
        F: Fn(Source::Value) -> T + ThreadSafe,
    {
        self.bindings.bind(destination, source, calculate);
    }

    /// Calculate and cache a value shared by multiple downstream bindings.
    pub fn derive<Source, T, F>(
        &mut self,
        source: Source,
        calculate: F,
    ) -> Derived<T>
    where
        Source: BindingSource<Graphics>,
        T: Clone + PartialEq + ThreadSafe,
        F: Fn(Source::Value) -> T + ThreadSafe,
    {
        self.bindings.derive(source, calculate)
    }

    /// Store and register an object in one operation. Dispatch is resolved
    /// statically from its Rust type, with no extra playback dispatch.
    pub fn add<T: SceneObject>(&mut self, object: T) -> T::Handle {
        object.insert_into(self)
    }

    fn insert_rectangle(
        &mut self,
        rectangle: Rectangle,
    ) -> RectangleHandle {
        let id = RectangleId(self.graphics.rectangles.len());
        self.graphics.rectangles.push(rectangle);
        self.graphics.order.push(DrawObject::Rectangle(id));
        RectangleHandle {
            id,
            position: Position::new(
                id,
                path!(Rectangle.position),
                path!(Rectangle.position.x),
                path!(Rectangle.position.y),
            ),
            fill: Property::new(id, path!(Rectangle.fill)),
            scale: Property::new(id, path!(Rectangle.scale)),
            rotation: Property::new(id, path!(Rectangle.rotation)),
        }
    }

    fn insert_text(&mut self, text: Text) -> TextHandle {
        let id = TextId(self.graphics.texts.len());
        self.graphics.texts.push(text);
        self.graphics.order.push(DrawObject::Text(id));
        TextHandle {
            id,
            content: Property::discrete(id, path!(Text.content)),
            position: Position::new(
                id,
                path!(Text.position),
                path!(Text.position.x),
                path!(Text.position.y),
            ),
            font_size: Property::new(id, path!(Text.font_size)),
            fill: Property::new(id, path!(Text.fill)),
            opacity: Property::new(id, path!(Text.opacity)),
            scale: Property::new(id, path!(Text.scale)),
            rotation: Property::new(id, path!(Text.rotation)),
        }
    }

    /// Build and bake once. The author callback does not execute in playback.
    /// Panics during construction on invalid bindings; use `try_animate` to
    /// handle errors instead. Binding calculations initialize at time zero.
    #[track_caller]
    pub fn animate(
        self,
        author: impl FnOnce(&mut Sequence<'_, Graphics>),
    ) -> Animation {
        self.try_animate(author).expect("invalid scene bindings")
    }

    /// Fallible finalization for applications that want to display binding
    /// errors and retain their last valid animation. No validation per frame.
    pub fn try_animate(
        mut self,
        author: impl FnOnce(&mut Sequence<'_, Graphics>),
    ) -> Result<Animation, BindingError> {
        let mut registry = Registry::new();
        let mut sequence = Sequence::new(&mut registry);
        author(&mut sequence);
        let mut timeline = sequence.compile();
        self.bindings.prepare(&self.graphics, &timeline)?;
        timeline.bake_actions(&registry, &self.graphics);
        let mut animation = Animation {
            path_geometry: self
                .graphics
                .paths
                .iter()
                .map(PathGeometry::new)
                .collect(),
            text_layouts: self
                .graphics
                .texts
                .iter()
                .map(TextLayout::new)
                .collect(),
            graphics: self.graphics,
            registry,
            timeline,
            bindings: self.bindings,
            updates: self.updates,
        };
        animation.sample(Duration::ZERO);
        Ok(animation)
    }
}

/// Registration contract for objects or reusable components added to a scene.
/// Built-in object implementations store their state and return typed handles;
/// custom components can also implement this by adding their constituent objects.
pub trait SceneObject {
    type Handle;
    fn insert_into(self, scene: &mut Scene) -> Self::Handle;
}

impl SceneObject for Rectangle {
    type Handle = RectangleHandle;

    fn insert_into(self, scene: &mut Scene) -> Self::Handle {
        scene.insert_rectangle(self)
    }
}

/// Owns the ordinary MotionGfx timeline and sampled graphics state.
pub struct Animation {
    graphics: Graphics,
    registry: Registry,
    timeline: Timeline<Graphics>,
    text_layouts: Vec<TextLayout>,
    path_geometry: Vec<PathGeometry>,
    bindings: BindingPlan<Graphics>,
    updates: Vec<updates::NativeUpdate>,
}

impl Animation {
    /// Current sampled camera state (position is local when attached).
    pub fn camera(&self) -> &Camera {
        &self.graphics.camera
    }

    pub fn duration(&self) -> Duration {
        self.timeline.tracks()[0].duration()
    }

    pub fn sample(&mut self, time: Duration) {
        self.timeline.set_target_time(time);
        self.graphics.sample_time = self.timeline.target_time();
        self.timeline.queue_actions();
        self.timeline.sample_queued_actions(
            &self.registry,
            &mut self.graphics,
        );
        self.bindings.evaluate(&mut self.graphics);
        for update in &mut self.updates {
            update.run(&mut self.graphics, &self.bindings);
        }
    }

    pub fn rectangle(&self, handle: RectangleHandle) -> &Rectangle {
        &self.graphics.rectangles[handle.id.0]
    }

    pub fn text(&self, handle: TextHandle) -> &Text {
        &self.graphics.texts[handle.id.0]
    }

    pub fn circle(&self, handle: CircleHandle) -> &Circle {
        &self.graphics.circles[handle.id.0]
    }

    pub fn path(&self, handle: PathHandle) -> &Path {
        &self.graphics.paths[handle.id.0]
    }

    /// Emit Vello commands from sampled state. No authoring descriptions are
    /// interpreted here. Vello still records its normal scene commands per frame.
    pub fn render(
        &mut self,
        scene: &mut vello::Scene,
        viewport: Size,
        canvas: Size,
        dpi: f64,
    ) {
        let fit = (viewport.width / canvas.width)
            .min(viewport.height / canvas.height);
        let screen = Affine::scale(dpi)
            * Affine::translate((
                viewport.width / 2.0,
                viewport.height / 2.0,
            ))
            * Affine::scale(fit)
            * self.graphics.camera_transform();
        for object in &self.graphics.order {
            match object {
                DrawObject::Line(id) => {
                    self.graphics.lines[id.0].render(scene, screen);
                }
                DrawObject::Circle(id) => {
                    let circle = &self.graphics.circles[id.0];
                    let parent =
                        self.graphics.parent_transform(circle.parent);
                    circle.render(scene, screen * parent);
                }
                DrawObject::Path(id) => self.path_geometry[id.0]
                    .render(
                        &self.graphics.paths[id.0],
                        scene,
                        screen,
                    ),
                DrawObject::Rectangle(id) => self.graphics.rectangles
                    [id.0]
                    .render(scene, screen),
                DrawObject::Text(id) => {
                    let text = &self.graphics.texts[id.0];
                    let parent =
                        self.graphics.text_parent_transform(text);
                    self.text_layouts[id.0].render(
                        text,
                        scene,
                        screen * parent,
                    );
                }
            }
        }
    }
}

impl Rectangle {
    fn transform(&self) -> Affine {
        Affine::translate((self.position.x, -self.position.y))
            * Affine::rotate(-self.rotation)
    }

    fn render(&self, scene: &mut vello::Scene, screen: Affine) {
        let rectangle = self;
        let xf = screen * self.transform();
        let half_w = rectangle.size.width * rectangle.scale / 2.0;
        let half_h = rectangle.size.height * rectangle.scale / 2.0;
        let shape = Rect::new(-half_w, -half_h, half_w, half_h)
            .to_rounded_rect(
                rectangle.radius * rectangle.scale.abs(),
            );
        scene.fill(Fill::NonZero, xf, rectangle.fill, None, &shape);
        if rectangle.stroke_width > 0.0 {
            scene.stroke(
                &Stroke::new(rectangle.stroke_width),
                xf,
                rectangle.stroke,
                None,
                &shape,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn authored_square_matches_direct_motiongfx_at_arbitrary_times() {
        let blue = Color::from_rgb8(88, 196, 221);
        let orange = Color::from_rgb8(255, 134, 47);
        let initial = Rectangle::from_size(Vec2::new(160.0, 160.0))
            .fill(blue)
            .stroke(blue, 4.0);
        let mut scene = Scene::new();
        let square = scene.add(initial.clone());
        let mut authored = scene.animate(|sequence| {
            sequence.play(square.position.offset((-80.0, 0.0)));
            sequence.play(square.fill.to(orange));
            sequence.play(square.scale.multiply(0.3));
            sequence.play(square.rotation.offset(0.4));
        });

        // Independent baseline: the original low-level builder, not the
        // convenience methods under test. Both use the same initial graphics.
        let mut world = Graphics {
            rectangles: vec![initial],
            ..Graphics::default()
        };
        let mut registry = Registry::new();
        let mut b = registry.create_builder::<Graphics>();
        let shift = b
            .act(square.id, path!(Rectangle.position), |p| {
                *p + (-80.0, 0.0)
            })
            .with_ease(smooth)
            .play(s(1));
        let fill = b
            .act(square.id, path!(Rectangle.fill), move |_| orange)
            .with_ease(smooth)
            .play(s(1));
        let scale = b
            .act(square.id, path!(Rectangle.scale), |s| s * 0.3)
            .with_ease(smooth)
            .play(s(1));
        let rotate = b
            .act(square.id, path!(Rectangle.rotation), |r| r + 0.4)
            .with_ease(smooth)
            .play(s(1));
        let mut timeline = b.compile(
            [shift, fill, scale, rotate].ord_chain().compile(),
        );
        timeline.bake_actions(&registry, &world);

        assert_eq!(authored.duration(), s(4));
        // Boundaries, eased midpoints, forward jumps and backwards scrubbing.
        for millis in [
            0, 250, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000,
            1200, 0,
        ] {
            let time = ms(millis);
            authored.sample(time);
            timeline.set_target_time(time);
            timeline.queue_actions();
            timeline.sample_queued_actions(&registry, &mut world);
            let actual = authored.rectangle(square);
            let expected = &world.rectangles[0];
            assert_eq!(actual.position, expected.position);
            assert_eq!(actual.fill, expected.fill);
            assert_eq!(actual.scale, expected.scale);
            assert_eq!(actual.rotation, expected.rotation);
            assert_eq!(actual.stroke, blue);
            assert_eq!(actual.stroke_width, 4.0);
        }
        authored.sample(s(4));
        let final_square = authored.rectangle(square);
        assert_eq!(final_square.position, Point::new(-80.0, 0.0));
        assert!((final_square.scale - 0.3).abs() < 1e-6);
        assert!((final_square.rotation - 0.4).abs() < 1e-6);
    }

    #[test]
    fn relative_targets_waits_overrides_and_multiple_objects() {
        let mut scene = Scene::new();
        let left = scene.add(Rectangle::default());
        let right = scene.add(Rectangle::default());
        let mut animation = scene.animate(|sequence| {
            sequence.defaults(s(1), ease::linear);
            sequence.play(
                left.position.offset((10.0, 0.0)).over(ms(200)),
            );
            sequence.wait(ms(300));
            sequence.play(
                left.position
                    .offset((5.0, 0.0))
                    .over(ms(500))
                    .ease(ease::quad::ease_in),
            );
            sequence.play_all([
                left.scale.to(2.0).over(ms(400)),
                right.scale.to(3.0).over(ms(800)),
            ]);
        });
        assert_eq!(animation.duration(), ms(1800));
        animation.sample(ms(400));
        assert_eq!(animation.rectangle(left).position.x, 10.0);
        animation.sample(ms(750));
        assert!(
            (animation.rectangle(left).position.x - 11.25).abs()
                < 1e-6
        );
        animation.sample(ms(1400));
        assert_eq!(animation.rectangle(left).position.x, 15.0);
        assert_eq!(animation.rectangle(right).position, Point::ZERO);
        assert_eq!(animation.rectangle(left).scale, 2.0);
        assert_eq!(animation.rectangle(right).scale, 2.0);
        animation.sample(Duration::ZERO);
        assert_eq!(animation.rectangle(left).position, Point::ZERO);
        assert_eq!(animation.rectangle(right).scale, 1.0);
    }

    #[test]
    fn custom_target_runs_during_baking_not_playback() {
        let mut scene = Scene::new();
        let square = scene.add(Rectangle::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let mut animation = scene.animate(|sequence| {
            sequence.play(square.scale.map_target(move |scale| {
                counter.fetch_add(1, Ordering::Relaxed);
                scale * 2.0
            }));
        });
        let baked_calls = calls.load(Ordering::Relaxed);
        assert!(baked_calls > 0);
        for time in [ms(0), ms(500), ms(1000), ms(250), ms(0)] {
            animation.sample(time);
        }
        assert_eq!(calls.load(Ordering::Relaxed), baked_calls);
    }

    #[test]
    fn nested_axes_and_whole_position_operations_share_the_same_state()
     {
        let mut scene = Scene::new();
        let shape = scene
            .add(Rectangle::new(10.0, 10.0).position((10.0, 20.0)));
        let title =
            scene.add(Text::new("Title").position((-3.0, 4.0)));
        let mut animation = scene.animate(|sequence| {
            sequence.defaults(s(1), ease::linear);
            sequence.parallel(|sequence| {
                sequence.play(shape.position.x.to(30.0));
                sequence.play(shape.position.y.offset(10.0));
                sequence.play(title.position.to((100.0, 50.0)));
            });
            sequence.play(shape.position.offset((5.0, -10.0)));
        });
        animation.sample(ms(500));
        assert_eq!(
            animation.rectangle(shape).position,
            Point::new(20.0, 25.0)
        );
        assert_eq!(
            animation.text(title).position,
            Point::new(48.5, 27.0)
        );
        animation.sample(ms(1500));
        assert_eq!(
            animation.rectangle(shape).position,
            Point::new(32.5, 25.0)
        );
        assert_eq!(
            animation.text(title).position,
            Point::new(100.0, 50.0)
        );
        animation.sample(ms(0));
        assert_eq!(
            animation.rectangle(shape).position,
            Point::new(10.0, 20.0)
        );
        assert_eq!(
            animation.text(title).position,
            Point::new(-3.0, 4.0)
        );
    }

    #[test]
    fn text_style_animation_and_content_changes_are_seekable() {
        let mut scene = Scene::new();
        let title = scene.add(Text::new("First").font_size(14.0));
        let orange = colors::ORANGE;
        let mut animation = scene.animate(|sequence| {
            sequence.defaults(s(1), ease::linear);
            sequence.parallel(|sequence| {
                sequence.play(title.position.x.to(100.0));
                sequence.play(title.font_size.to(28.0));
                sequence.play(title.opacity.to(0.5));
                sequence.play(title.scale.to(2.0));
                sequence.play(title.rotation.to(0.4));
                sequence.play(title.fill.to(orange));
                sequence.play(title.content.to("Second"));
            });
            sequence.wait(ms(500));
            sequence.play(title.content.set("Third"));
            sequence.wait(ms(500));
        });
        assert_eq!(animation.duration(), s(2));
        for (time, content) in [
            (0, "First"),
            (500, "First"),
            (1000, "Second"),
            (1499, "Second"),
            (1500, "Third"),
            (2000, "Third"),
            (500, "First"),
            (0, "First"),
        ] {
            animation.sample(ms(time));
            assert_eq!(
                animation.text(title).content.as_ref(),
                content
            );
        }
        animation.sample(ms(500));
        let text = animation.text(title);
        assert_eq!(text.position.x, 50.0);
        assert_eq!(text.font_size, 21.0);
        assert_eq!(text.opacity, 0.75);
        assert_eq!(text.scale, 1.5);
        assert!((text.rotation - 0.2).abs() < 1e-6);
        animation.sample(s(1));
        for (a, b) in animation
            .text(title)
            .fill
            .components
            .iter()
            .zip(orange.components)
        {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn mixed_text_and_rectangle_objects_render_in_insertion_order() {
        let mut scene = Scene::new();
        scene.add(Rectangle::new(10.0, 10.0).fill(colors::RED));
        scene.add(Text::new("A").font_size(7.0));
        scene.add(Rectangle::new(20.0, 20.0).fill(colors::BLUE));
        let mut animation =
            scene.animate(|sequence| sequence.wait(s(1)));
        let mut actual = vello::Scene::new();
        let size = Size::new(100.0, 100.0);
        animation.render(&mut actual, size, size, 1.0);

        let mut expected = vello::Scene::new();
        let screen = Affine::translate((50.0, 50.0));
        expected.fill(
            Fill::NonZero,
            screen,
            colors::RED,
            None,
            &Rect::new(-5.0, -5.0, 5.0, 5.0).to_rounded_rect(0.0),
        );
        lettering::draw(
            &mut expected,
            screen,
            Point::ZERO,
            "A",
            Color::WHITE,
            1.0,
        );
        expected.fill(
            Fill::NonZero,
            screen,
            colors::BLUE,
            None,
            &Rect::new(-10.0, -10.0, 10.0, 10.0).to_rounded_rect(0.0),
        );
        assert_eq!(
            actual.encoding().draw_data,
            expected.encoding().draw_data
        );
        assert_eq!(
            actual.encoding().path_data,
            expected.encoding().path_data
        );
        assert_eq!(actual.encoding().n_paths, 3);
    }

    #[test]
    fn anchored_text_inherits_parent_motion_and_keeps_independent_local_animation()
     {
        let mut scene = Scene::new();
        let bar = scene
            .add(Rectangle::new(10.0, 20.0).position((10.0, 20.0)));
        let label = scene.add(
            Text::new("A")
                .relative_to(bar)
                .position((0.0, -10.0))
                .font_size(7.0),
        );
        let mut animation = scene.animate(|sequence| {
            sequence.defaults(s(1), ease::linear);
            // No animation is registered on the child during this first second.
            sequence.parallel(|sequence| {
                sequence.play(bar.position.x.to(30.0));
                sequence.play(
                    bar.rotation.to(std::f64::consts::FRAC_PI_2),
                );
                sequence.play(bar.scale.to(2.0));
            });
            sequence.play(label.position.x.to(5.0));
        });
        let mut rendered = vello::Scene::new();
        for (time, expected) in [
            (0, Point::new(10.0, -10.0)),
            (1000, Point::new(50.0, -20.0)),
            (1500, Point::new(50.0, -25.0)),
            (2000, Point::new(50.0, -30.0)),
            (0, Point::new(10.0, -10.0)),
        ] {
            animation.sample(ms(time));
            let text = animation.text(label);
            let anchor =
                animation.graphics.text_parent_transform(text)
                    * Point::new(text.position.x, -text.position.y);
            assert!(anchor.distance(expected) < 1e-6);
            assert_eq!(text.position.y, -10.0);
            rendered.reset();
            animation.render(
                &mut rendered,
                Size::new(100.0, 100.0),
                Size::new(100.0, 100.0),
                1.0,
            );
            assert_eq!(rendered.encoding().n_paths, 2);
            assert_eq!(
                animation.text_layouts[label.id.0].rebuilds,
                1
            );
        }
    }
}
