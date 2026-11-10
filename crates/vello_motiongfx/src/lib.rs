//! Minimal native graphics integration. Object state is stored once; the
//! authoring handles only construct existing MotionGfx actions.

use motiongfx::prelude::*;
use motiongfx_authoring::{Property, Sequence};
use peniko_motiongfx as _;
use std::time::Duration;
use vello::kurbo::{Affine, Rect, Stroke};
pub use vello::kurbo::{Point, Size};
pub use vello::peniko::Color;
use vello::peniko::Fill;

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
        }
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
    pub position: Property<RectangleId, Rectangle, Point>,
    pub fill: Property<RectangleId, Rectangle, Color>,
    pub scale: Property<RectangleId, Rectangle, f64>,
    pub rotation: Property<RectangleId, Rectangle, f64>,
}

#[derive(Default)]
pub struct Graphics {
    rectangles: Vec<Rectangle>,
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

/// Creates graphics objects once, in insertion/draw order.
#[derive(Default)]
pub struct Drawing {
    graphics: Graphics,
}

impl Drawing {
    pub fn new() -> Self {
        Self::default()
    }

    /// Explicit object creation; returned fields describe animation targets.
    pub fn rectangle(
        &mut self,
        rectangle: Rectangle,
    ) -> RectangleHandle {
        let id = RectangleId(self.graphics.rectangles.len());
        self.graphics.rectangles.push(rectangle);
        RectangleHandle {
            id,
            position: Property::new(id, path!(Rectangle.position)),
            fill: Property::new(id, path!(Rectangle.fill)),
            scale: Property::new(id, path!(Rectangle.scale)),
            rotation: Property::new(id, path!(Rectangle.rotation)),
        }
    }

    /// Build and bake once. The callback does not execute during playback.
    pub fn animate(
        self,
        author: impl FnOnce(&mut Sequence<'_, Graphics>),
    ) -> Animation {
        let mut registry = Registry::new();
        let mut sequence = Sequence::new(&mut registry);
        author(&mut sequence);
        let mut timeline = sequence.compile();
        timeline.bake_actions(&registry, &self.graphics);
        Animation {
            graphics: self.graphics,
            registry,
            timeline,
        }
    }
}

/// Owns the ordinary MotionGfx timeline and sampled graphics state.
pub struct Animation {
    graphics: Graphics,
    registry: Registry,
    timeline: Timeline<Graphics>,
}

impl Animation {
    pub fn duration(&self) -> Duration {
        self.timeline.tracks()[0].duration()
    }

    pub fn sample(&mut self, time: Duration) {
        self.timeline.set_target_time(time);
        self.timeline.queue_actions();
        self.timeline.sample_queued_actions(
            &self.registry,
            &mut self.graphics,
        );
    }

    pub fn rectangle(&self, handle: RectangleHandle) -> &Rectangle {
        &self.graphics.rectangles[handle.id.0]
    }

    /// Emit Vello commands from sampled state. No authoring descriptions are
    /// interpreted here. Vello still records its normal scene commands per frame.
    pub fn render(
        &self,
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
            * Affine::scale(fit);
        for rectangle in &self.graphics.rectangles {
            let xf = screen
                * Affine::translate((
                    rectangle.position.x,
                    -rectangle.position.y,
                ))
                * Affine::rotate(-rectangle.rotation);
            let half_w = rectangle.size.width * rectangle.scale / 2.0;
            let half_h =
                rectangle.size.height * rectangle.scale / 2.0;
            let shape = Rect::new(-half_w, -half_h, half_w, half_h);
            scene.fill(
                Fill::NonZero,
                xf,
                rectangle.fill,
                None,
                &shape,
            );
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
}

/// Manim's normalized sigmoid easing, useful for source comparisons.
pub fn smooth(t: f32) -> f32 {
    let error = 1.0 / (1.0 + 5.0_f32.exp());
    let sigmoid = 1.0 / (1.0 + (-10.0 * (t - 0.5)).exp());
    ((sigmoid - error) / (1.0 - 2.0 * error)).clamp(0.0, 1.0)
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
        let initial = Rectangle {
            size: Size::new(160.0, 160.0),
            fill: blue,
            stroke: blue,
            stroke_width: 4.0,
            ..Rectangle::default()
        };
        let mut drawing = Drawing::new();
        let square = drawing.rectangle(initial.clone());
        let mut authored = drawing.animate(|sequence| {
            sequence.defaults(s(1), smooth);
            sequence.play(square.position.offset((-80.0, 0.0)));
            sequence.play(square.fill.to(orange));
            sequence.play(square.scale.multiply(0.3));
            sequence.play(square.rotation.offset(0.4));
        });

        // Independent baseline: the original low-level builder, not the
        // convenience methods under test. Both use the same initial graphics.
        let mut world = Graphics {
            rectangles: vec![initial],
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
        let mut drawing = Drawing::new();
        let left = drawing.rectangle(Rectangle::default());
        let right = drawing.rectangle(Rectangle::default());
        let mut animation = drawing.animate(|sequence| {
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
            let a = sequence.action(left.scale.to(2.0).over(ms(400)));
            let b =
                sequence.action(right.scale.to(3.0).over(ms(800)));
            sequence.append([a, b].ord_all());
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
        let mut drawing = Drawing::new();
        let square = drawing.rectangle(Rectangle::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let mut animation = drawing.animate(|sequence| {
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
}
