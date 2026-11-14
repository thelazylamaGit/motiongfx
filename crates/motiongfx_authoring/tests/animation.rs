//! `Animation` values compiled through `Scene::compile`: group timing against
//! hand-computed schedules (including backward seeks), the timing rules,
//! and error reporting. No renderer needed.

use motiongfx::prelude::*;
use motiongfx_authoring::{
    Animation, AnimationError, AnimationErrorKind, Property, Scene,
    SceneError, Spring, World, all, chain, flow, marker, signal,
    wait,
};
use std::time::Duration;

#[derive(Clone, Default, Debug, PartialEq)]
struct Item {
    x: f64,
    y: f64,
    caption: String,
}

#[derive(Default)]
struct Items(Vec<Item>);

impl SubjectSource<usize, Item> for World<Items> {
    fn get_source(&self, id: usize) -> Option<&Item> {
        self.model().0.get(id)
    }
    fn apply_source<R>(
        &mut self,
        id: usize,
        f: impl FnOnce(&mut Item) -> R,
    ) -> Option<R> {
        self.model_mut().0.get_mut(id).map(f)
    }
}

fn x(id: usize) -> Property<usize, Item, f64> {
    Property::new(id, path!(Item.x))
}
fn y(id: usize) -> Property<usize, Item, f64> {
    Property::new(id, path!(Item.y))
}
fn caption() -> Property<usize, Item, String> {
    Property::discrete(0, path!(Item.caption))
}

/// Three items, linear easing and 100 ms default timing.
fn scene() -> Scene<Items> {
    let mut scene =
        Scene::with_model(Items(vec![Item::default(); 3]));
    scene.defaults(ms(100), ease::linear);
    scene
}

fn item(
    animation: &mut motiongfx_authoring::CompiledScene<Items>,
    time: u64,
    id: usize,
) -> Item {
    animation.sample(ms(time));
    animation.model().0[id].clone()
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{actual} != {expected}"
    );
}

#[test]
fn debug_shows_the_tree_and_where_each_change_was_written() {
    let progress = signal(0.0);
    let line = line!() + 2;
    let animation: Animation<Items> =
        chain([progress.to(1.0), wait(s(1)), marker("end")]);
    let debug = format!("{animation:?}");
    let step = format!("Chain([Step({}:{line}:", file!());
    assert!(debug.starts_with(&step), "{debug}");
    assert!(
        debug.ends_with("), Wait(1s), Marker(\"end\")])"),
        "{debug}"
    );
}

#[test]
fn groups_follow_core_ordering_forwards_and_backwards() {
    let animation = scene().compile([
        caption().set("Start"),
        x(0).offset(10.0), // 0..100: 0 -> 10
        wait(ms(50)),      // 100..150
        all([
            x(0).offset(20.0).over(ms(300)), // 150..450: 10 -> 30
            chain([
                y(0).to(10.0),                   // 150..250: 0 -> 10
                y(0).offset(10.0).over(ms(200)), // 250..450: 10 -> 20
            ]),
        ]),
        flow(ms(50), [x(1).offset(10.0), x(2).offset(10.0)]), // 450.., 500..
        caption().set("Done"), // at 600
        wait(ms(50)),          // until 650
    ]);
    let mut animation = animation;
    assert_eq!(animation.duration(), ms(650));

    // Forward, then backward and out of order.
    for time in [50, 300, 525, 650, 300, 50, 525] {
        let first = item(&mut animation, time, 0);
        let model = &animation.model().0;
        match time {
            50 => {
                close(first.x, 5.0);
                close(first.y, 0.0);
                assert_eq!(first.caption, "Start");
            }
            300 => {
                close(first.x, 20.0);
                close(first.y, 12.5);
            }
            525 => {
                close(first.x, 30.0);
                close(first.y, 20.0);
                close(model[1].x, 7.5);
                close(model[2].x, 2.5);
                assert_eq!(first.caption, "Start");
            }
            650 => {
                close(model[1].x, 10.0);
                close(model[2].x, 10.0);
                assert_eq!(first.caption, "Done");
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn own_timing_wins_over_scene_defaults_set_at_any_point() {
    let mut scene = scene();
    // Built before `defaults` changes: defaults are read at compile.
    let steps = [x(0).to(10.0), x(0).to(20.0).over(ms(100))];
    scene.defaults(ms(200), ease::linear);
    let mut animation = scene.compile(steps);

    assert_eq!(animation.duration(), ms(300));
    close(item(&mut animation, 100, 0).x, 5.0);
    close(item(&mut animation, 250, 0).x, 15.0);
}

#[test]
fn own_ease_wins_over_the_scene_default() {
    let mut animation = scene().compile([x(0)
        .to(10.0)
        .ease(|t| if t < 1.0 { 0.0 } else { 1.0 })]);
    close(item(&mut animation, 50, 0).x, 0.0);
    close(item(&mut animation, 100, 0).x, 10.0);
}

#[test]
fn delay_starts_later_and_counts_towards_the_group() {
    let mut animation = scene().compile([
        all([x(0).to(10.0), y(0).to(10.0).delay(ms(50))]),
        chain([x(1).to(10.0)]).delay(ms(100)),
    ]);
    // all: max(100, 50 + 100) = 150; then 100 delay + 100.
    assert_eq!(animation.duration(), ms(350));
    close(item(&mut animation, 50, 0).y, 0.0);
    close(item(&mut animation, 100, 0).y, 5.0);
    close(item(&mut animation, 200, 1).x, 0.0);
    close(item(&mut animation, 300, 1).x, 5.0);
}

#[test]
fn wait_inside_all_does_not_delay_siblings() {
    let mut animation =
        scene().compile([all([wait(ms(300)), x(0).to(10.0)])]);
    assert_eq!(animation.duration(), ms(300));
    close(item(&mut animation, 50, 0).x, 5.0);
}

#[test]
fn clones_are_separate_passes_starting_where_the_last_ended() {
    let step = x(0).offset(10.0);
    let pulse: Animation<Items> = chain([step.clone(), step]);
    let mut animation = scene().compile([pulse.clone(), pulse]);
    assert_eq!(animation.duration(), ms(400));
    close(item(&mut animation, 100, 0).x, 10.0);
    close(item(&mut animation, 400, 0).x, 40.0);
}

#[test]
fn instant_changes_ignore_default_durations() {
    let mut animation = scene().compile([
        caption().set("A"),
        x(0).set(5.0),
        x(0).to(15.0),
    ]);
    assert_eq!(animation.duration(), ms(100));
    let first = item(&mut animation, 50, 0);
    assert_eq!(first.caption, "A");
    close(first.x, 10.0);
}

#[test]
fn signals_register_when_compiled_and_can_spring() {
    let progress = signal(0.0_f64);
    let label = signal(String::new());
    let spring: Spring = Spring::timed(ms(500)).bounce(0.6).into();
    let mut animation = scene().compile([
        label.set("go".to_string()),
        progress.to(1.0).spring(spring),
    ]);
    assert_eq!(animation.duration(), ms(500));

    let mut peak = 0.0_f64;
    for time in 0..=500 {
        animation.sample(ms(time));
        peak = peak
            .max(*animation.state().signal_value(&progress).unwrap());
    }
    assert!(peak > 1.0, "a bouncy spring overshoots its target");
    close(*animation.state().signal_value(&progress).unwrap(), 1.0);
    assert_eq!(animation.state().signal_value(&label).unwrap(), "go");
}

#[test]
fn every_problem_is_reported_with_its_location() {
    let line = line!();
    let result = scene().try_compile([
        caption().to("b".to_string()).spring(Spring::physical()),
        x(0).to(1.0).spring(Spring::physical().mass(-1.0)),
    ]);
    let Err(SceneError::Animation(errors)) = result else {
        panic!("expected animation errors");
    };
    assert!(matches!(
        errors[..],
        [
            AnimationError {
                kind: AnimationErrorKind::DiscreteSpring,
                ..
            },
            AnimationError {
                kind: AnimationErrorKind::Spring(_),
                ..
            },
        ]
    ));
    for (offset, error) in (2..).zip(&errors) {
        assert!(error.location.file().ends_with("animation.rs"));
        assert_eq!(error.location.line(), line + offset);
    }
    let message = SceneError::Animation(errors).to_string();
    assert_eq!(message.lines().count(), 2);
    assert!(message.contains("animation.rs:"));
}

#[test]
#[should_panic(expected = "have no timing of")]
fn timing_on_a_group_panics_where_it_is_written() {
    let _ = all::<Items>([x(1).to(1.0)]).over(ms(10));
}

#[test]
#[should_panic(expected = "have no timing of")]
fn timing_on_a_wait_panics_where_it_is_written() {
    let _ = wait::<Items>(ms(10)).ease(ease::linear);
}

#[test]
#[should_panic(expected = "a spring sets its own timing")]
fn tween_timing_after_a_spring_panics() {
    let _: Animation<Items> =
        x(0).to(1.0).spring(Spring::physical()).over(ms(1));
}

#[test]
#[should_panic(expected = "a spring sets its own timing")]
fn a_spring_after_tween_timing_panics() {
    let _: Animation<Items> =
        x(0).to(1.0).ease(ease::linear).spring(Spring::physical());
}

#[test]
#[should_panic(expected = "a spring sets its own timing")]
fn a_spring_after_an_instant_set_panics() {
    let _: Animation<Items> =
        x(0).set(1.0).spring(Spring::physical());
}

#[test]
#[should_panic(expected = "animation.rs:")]
fn compile_panics_with_the_location() {
    scene().compile([caption()
        .to("b".to_string())
        .spring(Spring::physical())]);
}

/// Every animation stores only what its case needs; this catches a
/// field or inline payload that would grow every animation in a tree.
#[test]
fn animations_stay_small() {
    assert!(
        std::mem::size_of::<Animation<Items>>() <= 48,
        "Animation is {} bytes",
        std::mem::size_of::<Animation<Items>>()
    );
}

#[test]
fn empty_compiles_to_zero_length() {
    let animation = scene().compile(Vec::<Animation<Items>>::new());
    assert_eq!(animation.duration(), Duration::ZERO);
}

#[test]
fn markers_name_times_in_chains_groups_and_flows() {
    let animation = scene().compile([
        marker("start"),
        x(0).to(1.0),                       // 0..100
        marker("after_x"),                  // 100
        all([y(0).to(1.0), marker("all")]), // group start: 100
        flow(
            ms(50),
            [
                marker("first_slot"),  // next child starts at 200
                x(1).to(1.0),          // 200..300
                marker("second_slot"), // next child starts at 250
                x(2).to(1.0),          // 250..350
                marker("flow_end"), // no child follows: group end, 350
            ],
        ),
        chain([marker("delayed")]).delay(ms(25)), // 375
    ]);
    let times = |name| animation.marker(name).unwrap();
    assert_eq!(times("start"), ms(0));
    assert_eq!(times("after_x"), ms(100));
    assert_eq!(times("all"), ms(100));
    assert_eq!(times("first_slot"), ms(200));
    assert_eq!(times("second_slot"), ms(250));
    assert_eq!(times("flow_end"), ms(350));
    assert_eq!(times("delayed"), ms(375));
    assert_eq!(animation.marker("missing"), None);
    assert_eq!(animation.duration(), ms(375));
    // Sorted by time, ties in authoring order.
    let names = animation
        .markers()
        .iter()
        .map(|(name, _)| name.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "start",
            "after_x",
            "all",
            "first_slot",
            "second_slot",
            "flow_end",
            "delayed"
        ]
    );
}

#[test]
fn markers_take_no_time_or_slot() {
    let with = scene().compile([flow(
        ms(50),
        [x(0).to(1.0), marker("between"), x(1).to(1.0)],
    )]);
    let without =
        scene().compile([flow(ms(50), [x(0).to(1.0), x(1).to(1.0)])]);
    assert_eq!(with.duration(), without.duration());
    assert_eq!(with.marker("between"), Some(ms(50)));
}

#[test]
fn duplicate_marker_names_report_both_locations() {
    let line = line!();
    let repeated: Animation<Items> = marker("loop");
    let result = scene().try_compile([
        marker("loop"),
        x(0).to(1.0),
        repeated.clone(),
        repeated,
    ]);
    let Err(SceneError::Animation(errors)) = result else {
        panic!("expected duplicate marker errors");
    };
    assert_eq!(errors.len(), 2);
    for error in &errors {
        let AnimationErrorKind::DuplicateMarker { name, first } =
            &error.kind
        else {
            panic!("expected a duplicate marker, got {error}");
        };
        assert_eq!(name, "loop");
        assert_eq!(first.line(), line + 3);
        assert_eq!(error.location.line(), line + 1);
    }
    assert!(errors[0].to_string().contains("already used at"));
}

#[test]
fn overlapping_changes_to_one_property_report_both_lines() {
    let line = line!();
    let result = scene().try_compile([
        all([x(0).to(1.0), y(0).to(1.0), x(0).to(2.0)]),
        // Touching end to start is not an overlap.
        x(1).to(1.0),
        x(1).to(2.0),
        flow(ms(50), [x(2).to(1.0), x(2).to(2.0)]),
    ]);
    let Err(SceneError::Animation(errors)) = result else {
        panic!("expected overlap errors");
    };
    let mut found: Vec<_> = errors
        .iter()
        .map(|error| {
            let AnimationErrorKind::Overlap { field, later } =
                error.kind
            else {
                panic!("expected an overlap, got {error}");
            };
            assert!(field.ends_with('x'), "{field}");
            (error.location.line() - line, later.line() - line)
        })
        .collect();
    found.sort();
    // The all([..]) on line +2, and the flow on line +6.
    assert_eq!(found, [(2, 2), (6, 6)]);
    assert!(
        errors[0].to_string().contains("overlaps a later one at")
    );
}

#[test]
#[should_panic(expected = "have no timing of")]
fn timing_on_a_marker_panics() {
    let _ = marker::<Items>("loop").over(ms(10));
}
