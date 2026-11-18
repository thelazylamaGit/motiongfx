//! Voice handling, timing and playback control, checked on rendered
//! audio. No audio device: offline rendering, or a `Player` and `Mixer`
//! driven on one thread.
//!
//! Most tests play constant-level ("DC") samples, so a rendered frame's
//! value says exactly which voices are sounding.

use motiongfx::prelude::{ms, s};
use motiongfx_audio::fundsp::prelude32::{adsr_live, sine_hz};
use motiongfx_audio::{Instrument, Player, Sample, Score};
use motiongfx_authoring::{
    Animation, CompiledScene, Scene, all, chain, flow, wait,
};
use std::time::Duration;

const RATE: u32 = 48_000;

fn frames(time: Duration) -> usize {
    (time.as_secs_f64() * f64::from(RATE)).round() as usize
}

fn compile(
    steps: impl IntoIterator<Item = Animation<()>>,
) -> CompiledScene<()> {
    Scene::<()>::new().compile(steps)
}

/// The left channel of interleaved stereo.
fn left(audio: &[f32]) -> Vec<f32> {
    audio.iter().step_by(2).copied().collect()
}

/// A constant `level` lasting `length`.
fn dc(level: f32, length: Duration) -> Sample {
    Sample::mono(RATE, vec![level; frames(length)])
}

fn rms(signal: &[f32]) -> f32 {
    (signal.iter().map(|s| s * s).sum::<f32>() / signal.len() as f32)
        .sqrt()
}

fn close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "{what}: {actual} != {expected}"
    );
}

/// A sine with a quick attack, full sustain and a 20 ms release.
fn sine() -> Instrument {
    Instrument::synth(|hz| {
        sine_hz(hz) * adsr_live(0.002, 0.0, 1.0, 0.02)
    })
}

#[test]
fn sounds_start_at_their_exact_frame() {
    let audio = left(
        &Score::new(&compile([chain([
            wait(ms(37)),
            dc(0.25, ms(10)).play(),
        ])]))
        .render(RATE),
    );
    let onset = frames(ms(37));
    assert_eq!(audio[onset - 1], 0.0);
    close(audio[onset], 0.25, "first frame");
    close(audio[onset + frames(ms(10)) - 1], 0.25, "last frame");
    assert_eq!(audio[onset + frames(ms(10)) + 1], 0.0);
}

#[test]
fn overlapping_plays_layer() {
    let click = dc(0.1, ms(100));
    let audio = left(
        &Score::new(&compile([all([
            click.play(),
            click.gain(2.0).play().delay(ms(50)),
        ])]))
        .render(RATE),
    );
    close(audio[frames(ms(25))], 0.1, "first alone");
    close(audio[frames(ms(75))], 0.3, "both");
    close(audio[frames(ms(125))], 0.2, "second alone");
}

#[test]
fn releasing_a_note_leaves_an_overlapping_same_pitch_note_sounding() {
    let sine = sine();
    let later = || sine.note(69).over(ms(600)).delay(ms(100));
    let both = Score::new(&compile([all([
        sine.note(69).over(ms(300)),
        later(),
    ])]));
    let alone = Score::new(&compile([later()]));
    // After the first note has been released and rung out, only the
    // second is sounding, exactly as if it had played alone.
    let window = frames(ms(450))..frames(ms(650));
    let both = rms(&left(&both.render(RATE))[window.clone()]);
    let alone = rms(&left(&alone.render(RATE))[window]);
    assert!(alone > 0.5, "the later note sounds ({alone})");
    close(both, alone, "rms with and without the earlier note");
}

#[test]
fn the_oldest_voice_is_stolen_beyond_the_limit() {
    // 33 long voices starting 1 ms apart; the first is much louder.
    let loud = dc(0.5, s(1));
    let quiet = dc(0.01, s(1));
    let plays = std::iter::once(loud.play())
        .chain((0..32).map(|_| quiet.play()));
    let audio = left(
        &Score::new(&compile([flow(ms(1), plays)])).render(RATE),
    );
    // 21 voices have started by 20 ms: the loud one and 20 quiet ones.
    close(
        audio[frames(ms(20))],
        0.5 + 20.0 * 0.01,
        "before the limit",
    );
    // The 33rd started at 32 ms and the first faded out over 5 ms.
    close(audio[frames(ms(100))], 32.0 * 0.01, "after stealing");
}

#[test]
fn stop_all_silences_and_a_resume_starts_only_later_events() {
    let scene = compile([all([
        dc(0.1, s(1)).play(),
        dc(0.2, ms(100)).play().delay(ms(500)),
    ])]);
    let (mut player, mut mixer) =
        Player::new(Score::new(&scene), RATE);
    let latency = frames(ms(30));

    player.advance(Duration::ZERO, ms(20));
    let mut playing = vec![0.0; 2 * frames(ms(100))];
    mixer.render(&mut playing);
    let playing = left(&playing);
    assert_eq!(playing[latency - 1], 0.0);
    close(playing[latency], 0.1, "playing");

    player.stop_all();
    let mut stopping = vec![0.0; 2 * frames(ms(10))];
    mixer.render(&mut stopping);
    let stopping = left(&stopping);
    let halfway = stopping[frames(Duration::from_micros(2500))];
    assert!(halfway > 0.0 && halfway < 0.1, "fading ({halfway})");
    assert!(stopping[frames(ms(5))..].iter().all(|&s| s == 0.0));

    // Seek to 400 ms and play on: the event at 500 ms starts 100 ms after
    // the resume; the one that began at 0 is not restarted.
    player.advance(ms(400), ms(600));
    let mut resumed = vec![0.0; 2 * frames(ms(300))];
    mixer.render(&mut resumed);
    let resumed = left(&resumed);
    let onset = latency + frames(ms(100));
    assert!(resumed[..onset].iter().all(|&s| s == 0.0));
    close(resumed[onset], 0.2, "only the later event");
}

#[test]
fn a_loop_wrap_keeps_the_spacing_between_events() {
    // 100 ms loop: events at 0 and 90 ms.
    let scene = compile([all([
        dc(0.1, ms(5)).play(),
        dc(0.2, ms(5)).play().delay(ms(90)),
        wait(ms(100)),
    ])]);
    let (mut player, mut mixer) =
        Player::new(Score::new(&scene), RATE);
    player.advance(Duration::ZERO, ms(50));
    player.advance(ms(50), ms(100));
    // The loop wraps back to the start.
    player.advance(Duration::ZERO, ms(50));
    let mut audio = vec![0.0; 2 * frames(ms(300))];
    mixer.render(&mut audio);
    let audio = left(&audio);
    let onsets: Vec<usize> = (1..audio.len())
        .filter(|&i| audio[i] != 0.0 && audio[i - 1] == 0.0)
        .collect();
    let first = frames(ms(30));
    assert_eq!(
        onsets,
        [first, first + frames(ms(90)), first + frames(ms(100))]
    );
}

#[test]
fn playing_backwards_starts_events_at_their_ends_in_reverse_order() {
    // A 100 ms event at 0 and a 50 ms one at 200 ms; play back from 400.
    let scene = compile([all([
        dc(0.1, ms(100)).play(),
        dc(0.2, ms(50)).play().delay(ms(200)),
        wait(ms(400)),
    ])]);
    let (mut player, mut mixer) =
        Player::new(Score::new(&scene), RATE);
    player.advance(ms(400), ms(300));
    player.advance(ms(300), ms(0));
    let mut audio = vec![0.0; 2 * frames(ms(600))];
    mixer.render(&mut audio);
    let audio = left(&audio);
    let latency = frames(ms(30));
    // The later event's end (250 ms) is reached 150 ms in, the earlier
    // one's (100 ms) 300 ms in; each plays forwards at its own length.
    let later = latency + frames(ms(150));
    let earlier = latency + frames(ms(300));
    assert_eq!(audio[later - 1], 0.0);
    close(audio[later], 0.2, "the later event first");
    assert_eq!(audio[later + frames(ms(50)) + 1], 0.0);
    assert_eq!(audio[earlier - 1], 0.0);
    close(audio[earlier], 0.1, "then the earlier one");
    close(
        audio[earlier + frames(ms(99))],
        0.1,
        "for its full length",
    );
}

#[test]
fn a_slower_speed_spreads_events_and_holds_them_longer() {
    // Events at 0 and 100 ms, 20 ms each, played at half speed.
    let scene = compile([all([
        dc(0.1, ms(20)).play(),
        dc(0.2, ms(20)).play().delay(ms(100)),
    ])]);
    let (mut player, mut mixer) =
        Player::new(Score::new(&scene), RATE);
    player.set_speed(0.5);
    player.advance(Duration::ZERO, ms(150));
    let mut audio = vec![0.0; 2 * frames(ms(400))];
    mixer.render(&mut audio);
    let audio = left(&audio);
    let latency = frames(ms(30));
    // The second event comes 200 ms of sound after the first.
    let second = latency + frames(ms(200));
    assert_eq!(audio[second - 1], 0.0);
    close(audio[second], 0.2, "the second event");
    // Samples keep their own length; a timeline cut would stretch.
    close(audio[latency + frames(ms(19))], 0.1, "the first sounds");
}

#[test]
fn volume_ramps_to_its_new_level() {
    let scene = compile([dc(0.4, s(1)).play()]);
    let (mut player, mut mixer) =
        Player::new(Score::new(&scene), RATE);
    player.set_volume(0.5);
    player.advance(Duration::ZERO, ms(10));
    let mut audio = vec![0.0; 2 * frames(ms(100))];
    mixer.render(&mut audio);
    let audio = left(&audio);
    let latency = frames(ms(30));
    // The gain began its ramp before the sound started, so the sound
    // starts already at the new level.
    close(audio[latency], 0.2, "half volume");
    player.set_volume(0.0);
    let mut later = vec![0.0; 2 * frames(ms(20))];
    mixer.render(&mut later);
    let later = left(&later);
    // The full range takes 10 ms, so half of it takes 5.
    assert!(later[frames(ms(2))] > 0.0, "still ramping");
    assert_eq!(later[frames(ms(6))], 0.0, "silent once ramped");
}

#[test]
fn offline_render_covers_the_scene_and_release_tails() {
    let sine = sine();
    let scene = compile([chain([
        wait(ms(200)),
        sine.note(69).over(ms(100)),
        wait(s(1)),
    ])]);
    let audio = Score::new(&scene).render(RATE);
    let end = frames(scene.duration());
    assert!(audio.len() / 2 >= end, "covers the scene");
    assert!(audio.len() / 2 < end + 64, "and stops when silent");
    let audio = left(&audio);
    assert!(audio[..frames(ms(200))].iter().all(|&s| s == 0.0));
    assert!(rms(&audio[frames(ms(210))..frames(ms(300))]) > 0.5);
    assert!(audio[frames(ms(400))..].iter().all(|&s| s.abs() < 1e-4));

    // A note at the very end rings on past the scene.
    let scene = compile([sine.note(69)]);
    let audio = Score::new(&scene).render(RATE);
    assert!(
        audio.len() / 2 > frames(scene.duration()) + frames(ms(15))
    );
}

#[test]
fn velocity_and_gain_scale_the_output() {
    let sine = sine();
    let peak = |note: Animation<()>| {
        let audio = left(&Score::new(&compile([note])).render(RATE));
        audio.iter().fold(0.0f32, |peak, s| peak.max(s.abs()))
    };
    let full = peak(sine.note(69));
    let half = peak(sine.velocity(0.5).note(69));
    assert!(full > 0.9, "the note sounds ({full})");
    close(half, full * 0.5, "velocity 0.5");
}

#[test]
#[should_panic(expected = "velocity must be between 0 and 1")]
fn an_out_of_range_velocity_panics() {
    let _ = sine().velocity(1.5);
}

#[test]
#[should_panic(expected = "at most one input")]
fn a_synth_with_two_inputs_panics() {
    use motiongfx_audio::fundsp::prelude32::pass;
    let _ = Instrument::synth(|_| pass() + pass());
}
