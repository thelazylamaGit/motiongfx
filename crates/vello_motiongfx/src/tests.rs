struct Baseline(super::Rectangle);
impl
    motiongfx::prelude::SubjectSource<
        super::RectangleId,
        super::Rectangle,
    > for Baseline
{
    fn get_source(
        &self,
        _: super::RectangleId,
    ) -> Option<&super::Rectangle> {
        Some(&self.0)
    }
    fn apply_source<R>(
        &mut self,
        _: super::RectangleId,
        f: impl FnOnce(&mut super::Rectangle) -> R,
    ) -> Option<R> {
        Some(f(&mut self.0))
    }
}

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
    let mut world = Baseline((*initial).clone());
    let mut registry = Registry::new();
    let mut b = registry.create_builder::<Baseline>();
    let shift = b
        .act(square.id(), path!(Rectangle.position), |p| {
            *p + (-80.0, 0.0)
        })
        .with_ease(smooth)
        .play(s(1));
    let fill = b
        .act(square.id(), path!(Rectangle.fill), move |_| orange)
        .with_ease(smooth)
        .play(s(1));
    let scale = b
        .act(square.id(), path!(Rectangle.scale), |s| s * 0.3)
        .with_ease(smooth)
        .play(s(1));
    let rotate = b
        .act(square.id(), path!(Rectangle.rotation), |r| r + 0.4)
        .with_ease(smooth)
        .play(s(1));
    let mut timeline =
        b.compile([shift, fill, scale, rotate].ord_chain().compile());
    timeline.bake_actions(&registry, &world);

    assert_eq!(authored.duration(), s(4));
    // Boundaries, eased midpoints, forward jumps and backwards scrubbing.
    for millis in [
        0, 250, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 1200,
        0,
    ] {
        let time = ms(millis);
        authored.sample(time);
        timeline.set_target_time(time);
        timeline.queue_actions();
        timeline.sample_queued_actions(&registry, &mut world);
        let actual = authored.rectangle(square);
        let expected = &world.0;
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
        sequence
            .play(left.position.offset((10.0, 0.0)).over(ms(200)));
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
        (animation.rectangle(left).position.x - 11.25).abs() < 1e-6
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
fn nested_axes_and_whole_position_operations_share_the_same_state() {
    let mut scene = Scene::new();
    let shape =
        scene.add(Rectangle::new(10.0, 10.0).position((10.0, 20.0)));
    let title = scene.add(Text::new("Title").position((-3.0, 4.0)));
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
    assert_eq!(animation.text(title).position, Point::new(-3.0, 4.0));
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
        assert_eq!(animation.text(title).content.as_ref(), content);
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
    let mut renderer = Renderer::default();
    let mut scene = Scene::new();
    scene.add(Rectangle::new(10.0, 10.0).fill(colors::RED));
    scene.add(Text::new("A").font_size(7.0));
    scene.add(Rectangle::new(20.0, 20.0).fill(colors::BLUE));
    let animation = scene.animate(|sequence| sequence.wait(s(1)));
    let mut actual = vello::Scene::new();
    let size = Size::new(100.0, 100.0);
    renderer.render(&animation, &mut actual, size, size, 1.0);

    let mut expected = vello::Scene::new();
    let screen = Affine::translate((50.0, 50.0));
    expected.fill(
        Fill::NonZero,
        screen,
        colors::RED,
        None,
        &Rect::new(-5.0, -5.0, 5.0, 5.0).to_rounded_rect(0.0),
    );
    let prepared = renderer.fonts_mut().layout(
        "A".into(),
        &motiongfx_text::LayoutStyle {
            size: 7.0,
            ..Default::default()
        },
    );
    for run in &prepared.runs {
        expected
            .draw_glyphs(&run.font)
            .font_size(run.font_size)
            .normalized_coords(&run.normalized_coords)
            .transform(screen)
            .brush(Color::WHITE)
            .draw(
                Fill::NonZero,
                run.glyphs.iter().map(|g| vello::Glyph {
                    id: g.id,
                    x: g.x,
                    y: g.y,
                }),
            );
    }
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
    assert_eq!(actual.encoding().resources.glyph_runs.len(), 1);
}

#[test]
fn anchored_text_inherits_parent_motion_and_keeps_independent_local_animation()
 {
    let mut renderer = Renderer::default();
    let mut scene = Scene::new();
    let bar =
        scene.add(Rectangle::new(10.0, 20.0).position((10.0, 20.0)));
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
            sequence
                .play(bar.rotation.to(std::f64::consts::FRAC_PI_2));
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
        let anchor = animation.model().text_parent_transform(text)
            * Point::new(text.position.x, -text.position.y);
        assert!(anchor.distance(expected) < 1e-6);
        assert_eq!(text.position.y, -10.0);
        rendered.reset();
        renderer.render(
            &animation,
            &mut rendered,
            Size::new(100.0, 100.0),
            Size::new(100.0, 100.0),
            1.0,
        );
        assert_eq!(rendered.encoding().resources.glyph_runs.len(), 1);
        assert_eq!(
            renderer.text_layouts[label.id().index()].fonts.builds(),
            1
        );
    }
}
