//! Two independent model adapters share one schedule, source graph and clock.
use motiongfx::prelude::*;
use motiongfx_authoring::{
    Animation, BindingPlan, BindingSource, Input, NativeUpdate,
    ObjectDefinition, Property, Scene, SceneObject, UpdateTargets,
    World, computed, signal,
};

#[derive(Clone, Default)]
struct PointModel {
    x: f64,
    y: f64,
}
#[derive(Clone, Default)]
struct DepthModel {
    z: f64,
    alpha: f64,
}
#[derive(Default)]
struct Models {
    point: PointModel,
    depth: DepthModel,
}
trait HasPoint {
    fn point(&self) -> &PointModel;
    fn point_mut(&mut self) -> &mut PointModel;
}
impl HasPoint for Models {
    fn point(&self) -> &PointModel {
        &self.point
    }
    fn point_mut(&mut self) -> &mut PointModel {
        &mut self.point
    }
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
struct PointId;
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
struct DepthId;
// Model-owned typed IDs make these impls legal across crate boundaries, even
// when World is owned by authoring and SubjectSource by the core crate.
impl<M: HasPoint> SubjectSource<PointId, PointModel> for World<M> {
    fn get_source(&self, _: PointId) -> Option<&PointModel> {
        Some(self.model().point())
    }
    fn apply_source<R>(
        &mut self,
        _: PointId,
        f: impl FnOnce(&mut PointModel) -> R,
    ) -> Option<R> {
        Some(f(self.model_mut().point_mut()))
    }
}
impl SubjectSource<DepthId, DepthModel> for World<Models> {
    fn get_source(&self, _: DepthId) -> Option<&DepthModel> {
        Some(&self.model().depth)
    }
    fn apply_source<R>(
        &mut self,
        _: DepthId,
        f: impl FnOnce(&mut DepthModel) -> R,
    ) -> Option<R> {
        Some(f(&mut self.model_mut().depth))
    }
}
#[derive(Clone, Copy)]
struct PointHandle {
    x: Property<PointId, PointModel, f64>,
}
impl<M: 'static + HasPoint> SceneObject<M> for PointModel {
    type Handle = PointHandle;
    fn insert_into(self, scene: &mut Scene<M>) -> PointHandle {
        *scene.model_mut().point_mut() = self;
        PointHandle {
            x: Property::new(PointId, path!(PointModel.x)),
        }
    }
}
impl SceneObject<Models> for DepthModel {
    type Handle = (
        Property<DepthId, DepthModel, f64>,
        Property<DepthId, DepthModel, f64>,
    );
    fn insert_into(self, scene: &mut Scene<Models>) -> Self::Handle {
        scene.model_mut().depth = self;
        (
            Property::new(DepthId, path!(DepthModel.z)),
            Property::new(DepthId, path!(DepthModel.alpha)),
        )
    }
}
impl UpdateTargets<Models> for PointHandle {
    type Objects<'a> = &'a mut PointModel;
    fn callback<S, F>(
        self,
        source: S,
        mut update: F,
        _: &World<Models>,
    ) -> NativeUpdate<Models>
    where
        S: BindingSource<World<Models>>,
        F: for<'a> FnMut(&'a mut PointModel, S::Value) + ThreadSafe,
    {
        NativeUpdate::new(
            move |world, plan: &BindingPlan<World<Models>>| {
                let value = plan.read_source(world, &source).unwrap();
                update(&mut world.model_mut().point, value);
            },
        )
    }
}
fn build() -> Animation<Models> {
    let progress = signal(0.0);
    let mut scene = Scene::<Models>::new();
    let point = scene.add(
        ObjectDefinition::new(PointModel::default()).property(
            Input::from(&progress),
            path!(PointModel.x),
            |h: PointHandle| h.x,
        ),
    );
    let (z, alpha) = scene.add(DepthModel::default());
    let shared = computed!((progress) => progress * 3.0);
    scene.bind(z, (point.x, &shared), |(x, shared)| x + shared);
    scene.update(point, z, |point, z| point.y = z * 2.0);
    scene.animate(|sequence| {
        sequence.parallel(|sequence| {
            sequence.play(progress.to(10.0).ease(ease::linear));
            sequence.play(alpha.to(1.0).ease(ease::linear));
        });
        sequence.wait(s(1));
    })
}
#[test]
fn mixed_models_sample_shared_dependencies_updates_holds_and_backward_seeks()
 {
    let mut animation = build();
    assert_eq!(animation.duration(), s(2));
    for time in [ms(500), s(2), ms(250), ms(0)] {
        animation.sample(time);
        let progress = time.as_secs_f64().min(1.0) * 10.0;
        assert_eq!(animation.model().point.x, progress);
        assert_eq!(animation.model().depth.z, progress * 4.0);
        assert_eq!(animation.model().point.y, progress * 8.0);
        assert_eq!(animation.model().depth.alpha, progress / 10.0);
    }
}
#[test]
fn shared_lifecycle_rejects_model_binding_conflicts() {
    let mut scene = Scene::<Models>::new();
    let point = scene.add(PointModel::default());
    scene.bind(point.x, point.x, |x| x + 1.0);
    assert!(matches!(
        scene.try_animate(|_| {}),
        Err(motiongfx_authoring::SceneError::Binding(
            motiongfx_authoring::BindingError::Cycle
        ))
    ));
}
