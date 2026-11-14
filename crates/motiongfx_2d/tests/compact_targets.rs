use motiongfx::prelude::{ease, ms};
use motiongfx_2d::{
    Circle, CircleHandle, CompiledScene2dExt, Scene, TextHandle,
};
use std::mem::{needs_drop, size_of};

#[test]
fn built_in_targets_store_only_object_ids() {
    use motiongfx_2d::fields;
    assert_eq!(size_of::<fields::text::opacity::Marker>(), 0);
    assert_eq!(
        size_of::<fields::text::opacity::Target>(),
        size_of::<usize>()
    );
    assert!(!needs_drop::<TextHandle>());
    assert_eq!(size_of::<TextHandle>(), size_of::<usize>());
    assert_eq!(size_of::<CircleHandle>(), size_of::<usize>());
    assert_eq!(
        size_of::<motiongfx_2d::RectangleHandle>(),
        size_of::<usize>()
    );
    assert_eq!(
        size_of::<motiongfx_2d::LineHandle>(),
        size_of::<usize>()
    );
    assert_eq!(
        size_of::<motiongfx_2d::PathHandle>(),
        size_of::<usize>()
    );
    assert_eq!(
        size_of::<motiongfx_2d::TriangleHandle>(),
        size_of::<usize>()
    );
    assert_eq!(
        size_of::<motiongfx_2d::ChevronHandle>(),
        size_of::<usize>()
    );
    assert_eq!(size_of::<motiongfx_2d::CameraHandle>(), 0);
    assert_eq!(
        size_of::<motiongfx_2d::TextSelectionHandle>(),
        2 * size_of::<usize>()
    );
}

#[test]
fn immediate_helpers_preserve_timeline_time_and_backward_seeks() {
    use motiongfx_2d::{Text, colors};
    let mut scene = Scene::new();
    let label = scene.add(Text::new("Start"));
    let mut animation = scene.animate(|sequence| {
        sequence.wait(ms(100));
        sequence.play(label.set_content("Done"));
        sequence.play(label.set_fill(colors::ORANGE));
        sequence.wait(ms(100));
    });
    assert_eq!(animation.duration(), ms(200));
    for (time, content, fill) in [
        (0, "Start", colors::WHITE),
        (150, "Done", colors::ORANGE),
        (0, "Start", colors::WHITE),
    ] {
        animation.sample(ms(time));
        assert_eq!(animation.text(label).content.as_ref(), content);
        assert_eq!(animation.text(label).fill, fill);
    }
    let mut scene = Scene::new();
    let dot = scene.add(Circle::new(1.0));
    let animation = scene.animate(|sequence| {
        sequence.play(dot.fill_to(colors::ORANGE));
    });
    assert_eq!(animation.duration(), motiongfx::prelude::s(1));
}

#[test]
fn compact_fields_remain_sources_and_general_property_targets() {
    let mut scene = Scene::new();
    let a = scene.add(Circle::new(10.0));
    let b = scene.add(Circle::new(a.radius()).position(a.position()));
    let mut animation = scene.animate(|sequence| {
        sequence.defaults(ms(1000), ease::linear);
        sequence.parallel(|sequence| {
            sequence.play(a.radius().to(20.0));
            sequence.play(a.position_x().to(100.0));
        });
    });
    for (time, radius, x) in
        [(500, 15.0, 50.0), (1000, 20.0, 100.0), (0, 10.0, 0.0)]
    {
        animation.sample(ms(time));
        assert_eq!(animation.circle(b).radius, radius);
        assert_eq!(animation.circle(b).position.x, x);
    }
}
