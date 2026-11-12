//! Test the public macro from outside its defining crate. No renderer needed.

use motiongfx::prelude::*;
use motiongfx_authoring::{Property, Sequence, animate};
use std::time::Duration;

#[derive(Clone, Default, Debug, PartialEq)]
struct Item {
    x: f64,
    y: f64,
    caption: String,
}

struct World(Vec<Item>);

impl SubjectSource<usize, Item> for World {
    fn get_source(&self, id: usize) -> Option<&Item> {
        self.0.get(id)
    }
    fn apply_source<R>(
        &mut self,
        id: usize,
        f: impl FnOnce(&mut Item) -> R,
    ) -> Option<R> {
        self.0.get_mut(id).map(f)
    }
}

fn x(id: usize) -> Property<usize, Item, f64> {
    Property::new(id, path!(Item.x))
}

fn choreography(
    use_macro: bool,
) -> (Registry, Timeline<World>, World) {
    let world = World(vec![Item::default(); 3]);
    let mut registry = Registry::new();
    let mut sequence = Sequence::<World>::new(&mut registry);
    sequence.defaults(ms(100), ease::linear);
    let xs = [x(0), x(1), x(2)];
    let y = Property::new(0usize, path!(Item.y));
    let caption = Property::discrete(0usize, path!(Item.caption));

    if use_macro {
        animate!(&mut sequence, {
            caption.set("Start");
            xs[0].offset(10.0);
            wait(ms(50));
            parallel {
                xs[0].offset(20.0).over(ms(300));
                chain {
                    y.to(10.0);
                    y.offset(10.0).over(ms(200));
                };
            }
            stagger(ms(50), xs.map(|p| p.offset(10.0)));
            caption.set("Done");
            wait(ms(50));
        });
    } else {
        // Equivalent construction using pre-existing low-level combinators.
        sequence.play(caption.set("Start"));
        sequence.play(xs[0].offset(10.0));
        sequence.wait(ms(50));
        let a = sequence.action(xs[0].offset(20.0).over(ms(300)));
        let b = sequence.action(y.to(10.0));
        let c = sequence.action(y.offset(10.0).over(ms(200)));
        sequence.append([a, [b, c].ord_chain()].ord_all());
        let staggered = xs.map(|p| sequence.action(p.offset(10.0)));
        sequence.append(staggered.ord_flow(ms(50)));
        sequence.play(caption.set("Done"));
        sequence.wait(ms(50));
    }
    let mut timeline = sequence.compile();
    timeline.bake_actions(&registry, &world);
    (registry, timeline, world)
}

fn sample(
    registry: &Registry,
    timeline: &mut Timeline<World>,
    world: &mut World,
    time: Duration,
) {
    timeline.set_target_time(time);
    timeline.queue_actions();
    timeline.sample_queued_actions(registry, world);
}

#[test]
fn nested_groups_and_stagger_match_raw_ordering_when_seeking() {
    let (registry, mut timeline, mut world) = choreography(true);
    let (raw_registry, mut raw_timeline, mut raw_world) =
        choreography(false);
    assert_eq!(timeline.tracks()[0].duration(), ms(700));
    assert_eq!(raw_timeline.tracks()[0].duration(), ms(700));
    for time in [
        0, 99, 100, 149, 150, 200, 250, 449, 450, 475, 500, 550, 650,
        700, 125, 0,
    ] {
        sample(&registry, &mut timeline, &mut world, ms(time));
        sample(
            &raw_registry,
            &mut raw_timeline,
            &mut raw_world,
            ms(time),
        );
        assert_eq!(world.0, raw_world.0, "sample at {time}ms");
    }
    sample(&registry, &mut timeline, &mut world, ms(550));
    assert_eq!(world.0[0].x, 40.0);
    assert_eq!(world.0[1].x, 5.0);
    assert_eq!(world.0[2].x, 0.0);
    sample(&registry, &mut timeline, &mut world, ms(650));
    assert_eq!(world.0[0].caption, "Done");
}

fn reusable_helper(sequence: &mut Sequence<'_, World>) {
    animate!(sequence, {
        x(0).offset(1.0).over(ms(10));
    });
}

#[test]
fn context_and_expressions_evaluate_once_and_helpers_can_reuse_context()
 {
    let mut world = World(vec![Item::default(); 3]);
    let mut registry = Registry::new();
    let mut sequence = Sequence::<World>::new(&mut registry);
    let mut context_calls = 0;
    let mut expression_calls = 0;
    let __change = 3.0; // Caller names cannot collide with macro temporaries.
    animate!(
        {
            context_calls += 1;
            &mut sequence
        },
        {
            x(0).to({
                expression_calls += 1;
                __change
            })
            .over(ms(10));
            parallel([x(1).to(4.0), x(2).to(5.0)]);
        }
    );
    for _ in 0..2 {
        reusable_helper(&mut sequence);
    }
    // Plain Rust can continue scheduling after macro invocations, too.
    sequence.wait(ms(20));
    assert_eq!(context_calls, 1);
    assert_eq!(expression_calls, 1);
    let mut timeline = sequence.compile();
    assert_eq!(timeline.tracks()[0].duration(), ms(1050));
    timeline.bake_actions(&registry, &world);
    sample(&registry, &mut timeline, &mut world, ms(1050));
    assert_eq!(world.0[0].x, 5.0);
    assert_eq!(world.0[1].x, 4.0);
    assert_eq!(world.0[2].x, 5.0);
}

#[test]
fn empty_groups_and_collections_add_no_time() {
    let world = World(vec![Item::default(); 3]);
    let mut registry = Registry::new();
    let mut sequence = Sequence::<World>::new(&mut registry);
    let empty: [Property<usize, Item, f64>; 0] = [];
    animate!(&mut sequence, {});
    assert_eq!(sequence.staggered(ms(50), |_| {}), Duration::ZERO);
    animate!(&mut sequence, {
        parallel {};
        chain {}
        parallel(empty.map(|p| p.to(1.0)));
        stagger(ms(50), empty.map(|p| p.to(1.0)));
        wait(ms(10));
    });
    let mut timeline = sequence.compile();
    assert_eq!(timeline.tracks()[0].duration(), ms(10));
    timeline.bake_actions(&registry, &world);
}

fn nested_stagger(
    scoped: bool,
) -> (Registry, Timeline<World>, World) {
    let world = World(vec![Item::default(); 3]);
    let mut registry = Registry::new();
    let mut sequence = Sequence::<World>::new(&mut registry);
    sequence.defaults(ms(80), ease::linear);
    let y = Property::new(0usize, path!(Item.y));
    let caption = Property::discrete(0usize, path!(Item.caption));
    sequence.wait(ms(50));
    if scoped {
        let mut calls = 0;
        let duration = sequence.staggered(ms(100), |sequence| {
            calls += 1;
            sequence.chain(|sequence| {
                sequence.play(x(0).to(10.0).over(ms(300)));
                sequence.wait(ms(50));
            });
            sequence.parallel(|sequence| {
                sequence.play(x(1).to(20.0).over(ms(50)));
                sequence.play(caption.set("ready"));
            });
            sequence.staggered(ms(25), |sequence| {
                sequence.play(x(2).to(30.0));
                sequence.play(y.to(40.0).over(ms(20)));
            });
        });
        assert_eq!(calls, 1);
        // The first branch outlasts the last; the existing 50ms prefix is
        // excluded from this group's returned duration.
        assert_eq!(duration, ms(350));
    } else {
        let first = [
            sequence.action(x(0).to(10.0).over(ms(300))),
            TrackFragment::silent(ms(50)),
        ]
        .ord_chain();
        let second = [
            sequence.action(x(1).to(20.0).over(ms(50))),
            sequence.action(caption.set("ready")),
        ]
        .ord_all();
        let third = [
            sequence.action(x(2).to(30.0)),
            sequence.action(y.to(40.0).over(ms(20))),
        ]
        .ord_flow(ms(25));
        sequence.append([first, second, third].ord_flow(ms(100)));
    }
    sequence.play(x(2).offset(5.0).over(ms(100)));
    let mut timeline = sequence.compile();
    assert_eq!(timeline.tracks()[0].duration(), ms(500));
    timeline.bake_actions(&registry, &world);
    (registry, timeline, world)
}

#[test]
fn scoped_stagger_matches_core_flow_for_mixed_nested_groups_and_backward_seeks()
 {
    let (registry, mut timeline, mut world) = nested_stagger(true);
    let (
        reference_registry,
        mut reference_timeline,
        mut reference_world,
    ) = nested_stagger(false);
    for time in [
        0, 50, 100, 150, 175, 200, 250, 300, 330, 399, 400, 450, 500,
        250, 0,
    ] {
        sample(&registry, &mut timeline, &mut world, ms(time));
        sample(
            &reference_registry,
            &mut reference_timeline,
            &mut reference_world,
            ms(time),
        );
        assert_eq!(world.0, reference_world.0, "sample at {time}ms");
    }
}
