use motiongfx::prelude::{ease, ms};
use motiongfx_2d::{
    Animation2dExt, Circle, CircleHandle, Scene, TextHandle,
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
    assert_eq!(size_of::<TextHandle>(), 17 * size_of::<usize>());
    assert_eq!(size_of::<CircleHandle>(), 11 * size_of::<usize>());
}

#[test]
fn compact_fields_remain_sources_and_general_property_targets() {
    let mut scene = Scene::new();
    let a = scene.add(Circle::new(10.0));
    let b = scene.add(Circle::new(a.radius).position(a.position));
    let mut animation = scene.animate(|sequence| {
        sequence.defaults(ms(1000), ease::linear);
        sequence.parallel(|sequence| {
            sequence.play(a.radius.to(20.0));
            sequence.play(a.position.x.to(100.0));
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
