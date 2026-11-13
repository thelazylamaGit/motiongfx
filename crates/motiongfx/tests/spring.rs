use motiongfx::prelude::*;

#[derive(Clone, Default)]
struct State {
    x: f64,
}
#[derive(Default)]
struct World(State);
impl SubjectSource<u32, State> for World {
    fn get_source(&self, id: u32) -> Option<&State> {
        (id == 0).then_some(&self.0)
    }
    fn apply_source<R>(
        &mut self,
        id: u32,
        f: impl FnOnce(&mut State) -> R,
    ) -> Option<R> {
        (id == 0).then(|| f(&mut self.0))
    }
}
fn sample(
    timeline: &mut Timeline<World>,
    registry: &Registry,
    world: &mut World,
    time: core::time::Duration,
) {
    timeline.set_target_time(time);
    timeline.queue_actions();
    timeline.sample_queued_actions(registry, world);
}

#[test]
fn core_springs_match_the_public_sampler_and_reconstruct_chained_actions()
 {
    let settings = Spring::physical().damping(6.0);
    let curve = settings.prepare().unwrap();
    let mut registry = Registry::new();
    let mut world = World::default();
    let mut b = registry.create_builder::<World>();
    let there =
        b.spring(0, path!(State.x), |_| 100.0, settings).play();
    let back = b.spring(0, path!(State.x), |_| 0.0, settings).play();
    let mut timeline = b.compile([there, back].ord_chain().compile());
    timeline.bake_actions(&registry, &world);
    for time in [
        ms(250),
        curve.duration() + ms(250),
        ms(100),
        curve.duration() * 2,
        ms(0),
    ] {
        sample(&mut timeline, &registry, &mut world, time);
        let expected = if time < curve.duration() {
            100.0 * curve.sample(time).position
        } else {
            100.0
                * (1.0
                    - curve.sample(time - curve.duration()).position)
        };
        assert!((world.0.x - expected).abs() < 0.0001, "{time:?}");
    }
}

#[test]
fn overlapping_springs_follow_existing_later_action_wins_policy() {
    for spring_first in [true, false] {
        let mut registry = Registry::new();
        let mut builder = registry.create_builder::<World>();
        let (first, second) = if spring_first {
            (
                builder
                    .spring(
                        0,
                        path!(State.x),
                        |_| 100.0,
                        Spring::physical(),
                    )
                    .play(),
                builder
                    .act(0, path!(State.x), |_| 200.0)
                    .play(ms(500)),
            )
        } else {
            (
                builder
                    .act(0, path!(State.x), |_| 200.0)
                    .play(ms(500)),
                builder
                    .spring(
                        0,
                        path!(State.x),
                        |_| 100.0,
                        Spring::physical(),
                    )
                    .play(),
            )
        };
        let track = [first, second].ord_flow(ms(50)).compile();
        let clips: Vec<_> = track.bake_clips().collect();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].start, ms(50));
    }
}

#[test]
fn timed_springs_chain_at_the_requested_boundary_and_seek_backwards()
{
    let settings = Spring::timed(ms(500)).bounce(0.6);
    let curve = settings.prepare().unwrap();
    let mut registry = Registry::new();
    let mut world = World::default();
    let mut builder = registry.create_builder::<World>();
    let first = builder
        .spring(0, path!(State.x), |_| 100.0, settings)
        .play();
    let second =
        builder.spring(0, path!(State.x), |_| 0.0, settings).play();
    let track = [first, second].ord_chain().compile();
    assert_eq!(track.duration(), ms(1000));
    let mut timeline = builder.compile(track);
    timeline.bake_actions(&registry, &world);
    for time in
        [ms(250), ms(500), ms(600), ms(1000), ms(100), ms(600)]
    {
        sample(&mut timeline, &registry, &mut world, time);
        let expected = if time < ms(500) {
            100.0 * curve.sample(time).position
        } else {
            100.0 * (1.0 - curve.sample(time - ms(500)).position)
        };
        assert!((world.0.x - expected).abs() < 0.0001);
    }
}
