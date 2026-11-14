//! A portable 2D store and a second model share the actual authoring lifecycle.
use motiongfx::prelude::*;
use motiongfx_2d::{
    Circle, CompiledScene2dExt, Graphics, Model2d, Point, computed,
    signal,
};
use motiongfx_authoring::{Property, Scene, SceneObject, World};

#[derive(Default)]
struct Models {
    graphics: Graphics,
    depth: Depth,
}
impl Model2d for Models {
    fn graphics(&self) -> &Graphics {
        &self.graphics
    }
    fn graphics_mut(&mut self) -> &mut Graphics {
        &mut self.graphics
    }
}
#[derive(Clone, Default)]
struct Depth {
    z: f64,
    opacity: f64,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
struct DepthId;
impl SubjectSource<DepthId, Depth> for World<Models> {
    fn get_source(&self, _: DepthId) -> Option<&Depth> {
        Some(&self.model().depth)
    }
    fn apply_source<R>(
        &mut self,
        _: DepthId,
        f: impl FnOnce(&mut Depth) -> R,
    ) -> Option<R> {
        Some(f(&mut self.model_mut().depth))
    }
}
impl SceneObject<Models> for Depth {
    type Handle = (
        Property<DepthId, Depth, f64>,
        Property<DepthId, Depth, f64>,
    );
    fn insert_into(self, scene: &mut Scene<Models>) -> Self::Handle {
        scene.model_mut().depth = self;
        (
            Property::new(DepthId, path!(Depth.z)),
            Property::new(DepthId, path!(Depth.opacity)),
        )
    }
}

#[test]
fn portable_objects_and_other_models_share_sources_schedule_and_backward_seeking()
 {
    let progress = signal(0.0);
    let mut scene = Scene::<Models>::new();
    let dot = scene.add(
        Circle::new(10.0).position(progress.map(|p| (p * 10.0, 0.0))),
    );
    let (z, opacity) = scene.add(Depth::default());
    let shared =
        computed((dot.position_x(), &progress), |(x, p)| x + p);
    scene.bind(z, &shared, |v| v * 2.0);
    let follower =
        scene.add(Circle::new(3.0).position(z.map(|z| (0.0, z))));
    let mut animation = scene.animate(|sequence| {
        sequence.parallel(|sequence| {
            sequence.play(progress.to(1.0).ease(ease::linear));
            sequence.play(opacity.to(1.0).ease(ease::linear));
        });
        sequence.wait(s(1));
    });
    assert_eq!(animation.duration(), s(2));
    for time in [ms(500), s(2), ms(250), ms(0)] {
        animation.sample(time);
        let p = time.as_secs_f64().min(1.0);
        assert_eq!(
            animation.circle(dot).position,
            Point::new(p * 10.0, 0.0)
        );
        assert_eq!(animation.model().depth.z, p * 22.0);
        assert_eq!(animation.model().depth.opacity, p);
        assert_eq!(
            animation.circle(follower).position,
            Point::new(0.0, p * 22.0)
        );
    }
}
