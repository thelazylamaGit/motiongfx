//! Bindings run only when their inputs may have changed, and still follow
//! seeks in either direction.
use motiongfx::prelude::{ease, ms, s};
use motiongfx_2d::{Circle, Scene, computed, signal, wait};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A counter and a clone of it to move into a closure.
fn counter() -> (Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    (count.clone(), count)
}

#[test]
fn computed_values_recalculate_only_while_their_signal_animates() {
    let mut scene = Scene::new();
    scene.defaults(s(1), ease::linear);
    let progress = signal(0.0);
    let (calls, counted) = counter();
    let radius = computed(&progress, move |p| {
        counted.fetch_add(1, Ordering::Relaxed);
        1.0 + p
    });
    let dot = scene.add(Circle::new(&radius));
    let mut animation =
        scene.compile([wait(s(1)), progress.to(10.0), wait(s(2))]);
    // The first sample calculates everything once.
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    for (time, radius, total) in [
        (500, 1.0, 1),   // before the change: nothing written
        (1500, 6.0, 2),  // during it
        (1500, 6.0, 2),  // the same value again: no recalculation
        (2500, 11.0, 3), // passing its end writes the final value
        (3500, 11.0, 3), // after it: nothing written
        (0, 1.0, 4),     // a backward seek restores the start
    ] {
        animation.sample(ms(time));
        assert_eq!(animation.get(dot).radius, radius, "at {time}ms");
        assert_eq!(
            calls.load(Ordering::Relaxed),
            total,
            "at {time}ms"
        );
    }
}

#[test]
fn time_driven_values_follow_every_change_of_time() {
    let mut scene = Scene::new();
    let time = scene.time();
    let (calls, counted) = counter();
    let radius = computed(time, move |time| {
        counted.fetch_add(1, Ordering::Relaxed);
        time.as_secs_f64()
    });
    let dot = scene.add(Circle::new(&radius));
    let mut animation = scene.compile([wait(s(2))]);
    for (time, total) in [(500, 2), (500, 2), (1500, 3), (250, 4)] {
        animation.sample(ms(time));
        assert_eq!(
            animation.get(dot).radius,
            time as f64 / 1000.0,
            "at {time}ms"
        );
        assert_eq!(
            calls.load(Ordering::Relaxed),
            total,
            "at {time}ms"
        );
    }
}

#[test]
fn bind_mut_has_the_last_word_and_its_readers_see_it_at_once() {
    let mut scene = Scene::new();
    scene.defaults(s(1), ease::linear);
    let size = signal(3.0);
    let dot = scene.add(Circle::new(1.0));
    let follower = scene.add(Circle::new(1.0));
    // The timeline animates the radius, but `bind_mut` writes it after.
    scene.bind_mut(dot, &size, |dot, size| dot.radius = size * 2.0);
    scene
        .bind(follower.radius(), dot.radius(), |radius| radius + 1.0);
    let mut animation =
        scene.compile([dot.radius().to(10.0), size.to(5.0)]);
    for (time, radius) in
        [(500, 6.0), (1500, 8.0), (2000, 10.0), (0, 6.0)]
    {
        animation.sample(ms(time));
        assert_eq!(animation.get(dot).radius, radius, "at {time}ms");
        assert_eq!(
            animation.get(follower).radius,
            radius + 1.0,
            "at {time}ms"
        );
    }
}
