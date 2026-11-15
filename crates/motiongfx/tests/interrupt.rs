//! Interrupting clips: a clip marked with `TrackFragment::interrupting`
//! cuts a clip on the same field that is still running when it starts.
//! The cut clip keeps its original curve up to the cut, and the
//! interrupting clip starts from the value it reached.

use std::time::Duration;

use motiongfx::prelude::*;
use motiongfx::track::delay;

mod common;
use common::{
    CUBE, Transform, Vec3, World, lerp_f32, sample_at,
    world_starting_at,
};

/// A linear `translation.x` clip to `target` over `duration`.
fn slide_x(
    builder: &mut TimelineBuilder<'_, World>,
    target: f32,
    duration: Duration,
) -> TrackFragment {
    builder
        .act_builder(
            CUBE,
            path!(Transform.translation.x),
            move |_| target,
        )
        .with_interp(lerp_f32)
        .play(duration)
}

fn x_at(
    registry: &Registry,
    timeline: &mut Timeline<World>,
    world: &mut World,
    seconds: f32,
) -> f32 {
    sample_at(
        registry,
        timeline,
        world,
        Duration::from_secs_f32(seconds),
    )
    .x
}

fn close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "{what}: expected {expected}, got {actual}"
    );
}

/// Bakes `timeline` against a world starting at the origin.
fn bake(
    registry: &Registry,
    mut timeline: Timeline<World>,
) -> (Timeline<World>, World) {
    let world = world_starting_at(Vec3::default());
    timeline.bake_actions(registry, &world);
    (timeline, world)
}

#[test]
fn a_cut_clip_keeps_its_curve_and_the_interrupter_continues_from_it()
{
    let mut registry = Registry::new();
    let mut builder = registry.create_builder::<World>();
    // A: 0 -> 10 over [0, 4]. B: -> 20 over [1, 3], cutting A at 1,
    // where A has reached 2.5.
    let a = slide_x(&mut builder, 10.0, s(4));
    let b = slide_x(&mut builder, 20.0, s(2)).interrupting();
    let track = [a, delay(s(1), b)].ord_all();
    let timeline = builder.compile(track.compile());
    let (mut timeline, mut world) = bake(&registry, timeline);
    let mut x = |t| x_at(&registry, &mut timeline, &mut world, t);

    close(
        x(0.5),
        1.25,
        "A before the cut follows its original curve",
    );
    close(x(2.0), 11.25, "B starts from 2.5, halfway to 20");
    close(x(3.5), 20.0, "B's end");
    // Backward, across the cut and into A.
    close(x(0.8), 2.0, "A again after seeking back");
    close(x(2.5), 15.625, "B again after seeking forward");
}

#[test]
fn interruptions_chain() {
    let mut registry = Registry::new();
    let mut builder = registry.create_builder::<World>();
    // A: 0 -> 10 over [0, 4], cut at 1 (2.5). B: -> 20 over [1, 5], cut
    // at 2 (2.5 + 17.5 / 4 = 6.875). C: -> 0 over [2, 4].
    let a = slide_x(&mut builder, 10.0, s(4));
    let b = slide_x(&mut builder, 20.0, s(4)).interrupting();
    let c = slide_x(&mut builder, 0.0, s(2)).interrupting();
    let track = [a, delay(s(1), b), delay(s(2), c)].ord_all();
    let timeline = builder.compile(track.compile());
    let (mut timeline, mut world) = bake(&registry, timeline);
    let mut x = |t| x_at(&registry, &mut timeline, &mut world, t);

    close(x(1.5), 4.6875, "B, an eighth of the way from 2.5 to 20");
    close(x(3.0), 3.4375, "C, halfway from 6.875 to 0");
    close(x(4.5), 0.0, "C's end");
    close(x(1.5), 4.6875, "B again after seeking back");
}

#[test]
fn an_instant_interrupter_cuts_and_jumps() {
    let mut registry = Registry::new();
    let mut builder = registry.create_builder::<World>();
    let a = slide_x(&mut builder, 10.0, s(4));
    let b =
        slide_x(&mut builder, 20.0, Duration::ZERO).interrupting();
    let track = [a, delay(s(2), b)].ord_all();
    let timeline = builder.compile(track.compile());
    let (mut timeline, mut world) = bake(&registry, timeline);
    let mut x = |t| x_at(&registry, &mut timeline, &mut world, t);

    close(x(1.0), 2.5, "A before the jump");
    close(x(3.0), 20.0, "after the jump, A no longer runs");
    close(x(1.0), 2.5, "A again after seeking back");
}

#[test]
fn a_cut_spring_hands_over_the_value_it_reached() {
    let settings = Spring::physical();
    let curve = settings.prepare().unwrap();
    let cut = curve.duration() / 3;

    let mut registry = Registry::new();
    let mut builder = registry.create_builder::<World>();
    let a = builder
        .spring(
            CUBE,
            path!(Transform.translation.x),
            |_| 10.0,
            settings,
        )
        .play();
    let b = slide_x(&mut builder, 20.0, s(2)).interrupting();
    let track = [a, delay(cut, b)].ord_all();
    let timeline = builder.compile(track.compile());
    let (mut timeline, mut world) = bake(&registry, timeline);

    let reached = 10.0 * curve.sample(cut).position as f32;
    let before = cut / 2;
    let spring_before = 10.0 * curve.sample(before).position as f32;
    close(
        sample_at(&registry, &mut timeline, &mut world, before).x,
        spring_before,
        "the spring before the cut",
    );
    close(
        sample_at(&registry, &mut timeline, &mut world, cut + s(1)).x,
        reached + (20.0 - reached) * 0.5,
        "B halfway from where the spring was cut",
    );
}

#[test]
fn clips_starting_together_are_not_cut() {
    let mut registry = Registry::new();
    let mut builder = registry.create_builder::<World>();
    let a = slide_x(&mut builder, 10.0, s(4));
    let b = slide_x(&mut builder, 20.0, s(2)).interrupting();
    let track = [a, b].ord_all().compile();
    assert_no_cuts(&track);
}

#[test]
fn aliasing_fields_are_not_cut() {
    let mut registry = Registry::new();
    let mut builder = registry.create_builder::<World>();
    let a = builder
        .act_builder(CUBE, path!(Transform.translation), |_| Vec3 {
            x: 10.0,
            y: 10.0,
            z: 10.0,
        })
        .with_interp(common::lerp_vec3)
        .play(s(4));
    let b = slide_x(&mut builder, 20.0, s(2)).interrupting();
    let track = [a, delay(s(1), b)].ord_all().compile();
    assert_no_cuts(&track);
}

#[test]
fn an_interrupter_that_overlaps_nothing_changes_nothing() {
    let mut registry = Registry::new();
    let mut builder = registry.create_builder::<World>();
    let a = slide_x(&mut builder, 10.0, s(2));
    let b = slide_x(&mut builder, 20.0, s(2)).interrupting();
    let track = [a, b].ord_chain().compile();
    assert_no_cuts(&track);
}

#[test]
fn cutting_keeps_the_reserved_duration() {
    let mut registry = Registry::new();
    let mut builder = registry.create_builder::<World>();
    let a = slide_x(&mut builder, 10.0, s(4));
    let b = slide_x(&mut builder, 20.0, s(1)).interrupting();
    // A reserved [0, 4]; cut at 1, B ends at 2. `all` still lasts 4.
    let track = [a, delay(s(1), b)].ord_all().compile();
    assert_eq!(track.duration(), s(4));
}

fn assert_no_cuts(track: &Track) {
    for (_, span) in track.sequences_spans() {
        for index in 0..span.len {
            assert_eq!(track.cut(*span, index), None);
        }
    }
}
