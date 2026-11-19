use motiongfx::prelude::{ease, ms, s};
use motiongfx_2d::{
    AnimationError, AnimationErrorKind, CompileError, Parent, Point,
    Rect, Rectangle, Scene, Size, Text, Vec2, all, anchor, signal,
    wait,
};

#[test]
fn local_rect_puts_the_anchor_at_the_origin() {
    let mut rectangle = Rectangle::default();
    rectangle.size = Size::new(40.0, 20.0);
    assert_eq!(
        rectangle.local_rect(),
        Rect::new(-20.0, -10.0, 20.0, 10.0)
    );

    // Local y points down, so a bottom-anchored rectangle lies above (at
    // smaller y than) its anchor.
    rectangle.anchor = anchor::BOTTOM;
    assert_eq!(
        rectangle.local_rect(),
        Rect::new(-20.0, -20.0, 20.0, 0.0)
    );

    // Scale grows the rectangle away from its anchor.
    rectangle.anchor = anchor::TOP_RIGHT;
    rectangle.transform.scale = 2.0;
    assert_eq!(
        rectangle.local_rect(),
        Rect::new(-80.0, 0.0, 0.0, 40.0)
    );
}

#[test]
fn width_and_height_animate_separately_but_overlap_size() {
    let mut scene = Scene::new();
    scene.defaults(s(1), ease::linear);
    let bar = scene.add(Rectangle::new(10.0, 20.0));
    let mut animation = scene.compile([all([
        bar.height().to(60.0),
        bar.width().to(30.0),
    ])]);
    for (time, size) in [
        (500, Size::new(20.0, 40.0)),
        (1000, Size::new(30.0, 60.0)),
        (0, Size::new(10.0, 20.0)),
    ] {
        animation.sample(ms(time));
        assert_eq!(animation.get(bar).size, size, "at {time}ms");
    }

    let mut scene = Scene::new();
    let bar = scene.add(Rectangle::new(10.0, 20.0));
    let result = scene.try_compile([all([
        bar.size().to((1.0, 1.0)),
        bar.height().to(5.0),
    ])]);
    let Err(CompileError::Animation(errors)) = result else {
        panic!("expected an overlap error");
    };
    assert!(matches!(
        errors[..],
        [AnimationError {
            kind: AnimationErrorKind::Overlap { .. },
            ..
        }]
    ));
}

#[test]
fn anchor_animates_and_follows_a_signal() {
    let mut scene = Scene::new();
    scene.defaults(s(1), ease::linear);
    let bar = scene.add(Rectangle::new(10.0, 10.0));
    let mut animation =
        scene.compile([bar.anchor().to(anchor::BOTTOM_LEFT)]);
    for (time, anchor) in [
        (500, Vec2::new(-0.5, -0.5)),
        (1000, anchor::BOTTOM_LEFT),
        (0, anchor::CENTER),
    ] {
        animation.sample(ms(time));
        assert_eq!(animation.get(bar).anchor, anchor, "at {time}ms");
    }

    let mut scene = Scene::new();
    scene.defaults(s(1), ease::linear);
    let side = signal(anchor::LEFT);
    let bar = scene.add(Rectangle::new(10.0, 10.0).anchor(&side));
    let mut animation = scene.compile([side.to(anchor::RIGHT)]);
    for (time, anchor) in [
        (500, anchor::CENTER),
        (1000, anchor::RIGHT),
        (0, anchor::LEFT),
    ] {
        animation.sample(ms(time));
        assert_eq!(animation.get(bar).anchor, anchor, "at {time}ms");
    }
}

#[test]
fn children_hang_off_the_anchor_point() {
    let mut scene = Scene::new();
    let bar = scene.add(
        Rectangle::new(10.0, 40.0)
            .anchor(anchor::BOTTOM)
            .position((100.0, 0.0)),
    );
    let label = scene
        .add(Text::new("1").relative_to(bar).position((0.0, -10.0)));
    let animation = scene.compile([wait(ms(0))]);
    let canvas = animation.model();
    // The bar's position is its bottom edge, so a child at the origin
    // sits there, not at the bar's centre 20 units higher. Canvas
    // transforms flip y to point down.
    let origin = canvas.parent_transform(Some(Parent::from(bar)))
        * Point::ZERO;
    assert_eq!(origin, Point::new(100.0, 0.0));
    assert_eq!(
        animation.get(label).transform.position,
        Point::new(0.0, -10.0)
    );
}
