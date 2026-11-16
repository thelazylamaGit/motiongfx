//! Voices and the engine that mixes them, shared by live playback and
//! offline rendering. Once a voice is built, nothing here allocates, frees
//! or locks: voices move by value through preallocated queues and
//! vectors, and finished ones go to a `discard` callback, which the live
//! mixer uses to send them back to the host thread to be freed.

use crate::sound::Frames;
use std::sync::Arc;

/// Frames mixed at a time. Voices start and stop at exact frames inside a
/// block; the block only bounds how often new voices are started.
pub(crate) const BLOCK: usize = 64;
/// Voices that sound at once, across every kind of sound. Starting
/// another fades out the oldest.
pub(crate) const MAX_VOICES: usize = 32;
/// Room for the voices above plus ones still fading out.
const VOICE_SLOTS: usize = MAX_VOICES * 2;
/// Voices waiting for their start frame. Live playback only schedules a
/// few tens of milliseconds ahead, so this is rarely more than a handful.
pub(crate) const PENDING_SLOTS: usize = 256;
/// Output below this (−80 dB) counts as silence when deciding that a
/// released voice has finished ringing.
const SILENCE: f32 = 1e-4;

/// Where a voice's sound comes from.
pub(crate) enum Source {
    #[cfg(feature = "fundsp")]
    Synth {
        unit: Box<dyn fundsp::audiounit::AudioUnit>,
        /// Whether the graph reads a gate input (1 held, 0 released).
        gated: bool,
        stereo: bool,
        gate: f32,
    },
    /// Boxed: the synthesizer and its buffers are much larger than the
    /// other sources.
    #[cfg(feature = "rustysynth")]
    SoundFont(Box<SoundFontVoice>),
    Sample {
        frames: Arc<Frames>,
        /// Read position in the sample's own frames.
        position: f64,
        /// Sample frames per output frame.
        step: f64,
    },
}

/// One note on its own synthesizer, so its release can't end another
/// note of the same key.
#[cfg(feature = "rustysynth")]
pub(crate) struct SoundFontVoice {
    pub(crate) synth: rustysynth::Synthesizer,
    pub(crate) key: i32,
    pub(crate) left: [f32; BLOCK],
    pub(crate) right: [f32; BLOCK],
}

impl Source {
    /// Fill `out` (interleaved stereo, zeroed). Returns `false` once the
    /// source has nothing left to play.
    fn render(&mut self, out: &mut [f32]) -> bool {
        match self {
            #[cfg(feature = "fundsp")]
            Source::Synth {
                unit,
                gated,
                stereo,
                gate,
            } => {
                let gate = [*gate];
                let input: &[f32] = if *gated { &gate } else { &[] };
                let mut frame = [0.0; 2];
                let outputs = if *stereo { 2 } else { 1 };
                for out in out.as_chunks_mut::<2>().0 {
                    unit.tick(input, &mut frame[..outputs]);
                    out[0] = frame[0];
                    out[1] = frame[outputs - 1];
                }
                true
            }
            #[cfg(feature = "rustysynth")]
            Source::SoundFont(voice) => {
                let SoundFontVoice {
                    synth, left, right, ..
                } = &mut **voice;
                let frames = out.len() / 2;
                synth.render(
                    &mut left[..frames],
                    &mut right[..frames],
                );
                for (out, (l, r)) in out
                    .as_chunks_mut::<2>()
                    .0
                    .iter_mut()
                    .zip(left.iter().zip(right.iter()))
                {
                    out[0] = *l;
                    out[1] = *r;
                }
                true
            }
            Source::Sample {
                frames,
                position,
                step,
            } => {
                let samples = &frames.samples;
                let len = samples.len() / 2;
                for out in out.as_chunks_mut::<2>().0 {
                    // Truncation is the floor here: positions are never
                    // negative.
                    let index = *position as usize;
                    if index >= len {
                        return false;
                    }
                    let t = (*position - index as f64) as f32;
                    let here = &samples[2 * index..2 * index + 2];
                    let next = samples
                        .get(2 * index + 2..2 * index + 4)
                        .unwrap_or(&[0.0, 0.0]);
                    out[0] = here[0] + (next[0] - here[0]) * t;
                    out[1] = here[1] + (next[1] - here[1]) * t;
                    *position += *step;
                }
                true
            }
        }
    }

    /// Note-off. Returns `true` if the source has no release of its own
    /// and should fade out instead.
    fn release(&mut self) -> bool {
        match self {
            #[cfg(feature = "fundsp")]
            Source::Synth { gated, gate, .. } => {
                *gate = 0.0;
                !*gated
            }
            #[cfg(feature = "rustysynth")]
            Source::SoundFont(voice) => {
                voice.synth.note_off(0, voice.key);
                false
            }
            Source::Sample { .. } => true,
        }
    }
}

/// One sounding event: a source with its gain, start and release frames,
/// and the state that decides when it has finished.
pub(crate) struct Voice {
    source: Source,
    gain: f32,
    start: u64,
    release: u64,
    released: bool,
    /// Frames left in a fade-out, once the voice is stopping.
    fade: Option<u32>,
    fade_length: u32,
    /// Consecutive silent frames since the release.
    quiet: u32,
    quiet_limit: u32,
    /// Frames since the release.
    tail: u64,
    tail_limit: u64,
}

impl Voice {
    pub(crate) fn new(
        source: Source,
        gain: f32,
        start: u64,
        release: u64,
        rate: u32,
    ) -> Self {
        Self {
            source,
            gain,
            start,
            release: release.max(start),
            released: false,
            fade: None,
            // A 5 ms fade avoids the click of a hard cut.
            fade_length: (rate / 200).max(1),
            quiet: 0,
            // Silent for 50 ms after the release: it has finished ringing.
            quiet_limit: (rate / 20).max(1),
            tail: 0,
            // Cap a release that never falls silent at 10 s.
            tail_limit: u64::from(rate) * 10,
        }
    }

    /// Fade out over 5 ms, then finish.
    fn fade_out(&mut self) {
        if self.fade.is_none() {
            self.fade = Some(self.fade_length);
        }
    }

    /// Add this voice to `mix` (interleaved stereo, starting at frame
    /// `now`), using `scratch` as working space. Returns `false` once the
    /// voice has finished.
    fn mix(
        &mut self,
        now: u64,
        mix: &mut [f32],
        scratch: &mut [f32],
    ) -> bool {
        let frames = mix.len() / 2;
        // Frame offsets in this block; usize because they are at most
        // `BLOCK`.
        let begin =
            (self.start.saturating_sub(now) as usize).min(frames);
        let split = (self.release.saturating_sub(now) as usize)
            .clamp(begin, frames);
        let scratch = &mut scratch[..frames * 2];
        scratch.fill(0.0);

        let mut fade_from =
            if self.fade.is_some() { begin } else { frames };
        let mut playing = true;
        if begin < split {
            playing = self
                .source
                .render(&mut scratch[2 * begin..2 * split]);
        }
        if playing && split < frames {
            if !self.released {
                self.released = true;
                if self.source.release() && self.fade.is_none() {
                    self.fade_out();
                    fade_from = split;
                }
            }
            playing = self.source.render(&mut scratch[2 * split..]);
        }

        for index in begin..frames {
            let mut gain = self.gain;
            if index >= fade_from
                && let Some(left) = &mut self.fade
            {
                if *left == 0 {
                    return false;
                }
                gain *= *left as f32 / self.fade_length as f32;
                *left -= 1;
            }
            let (l, r) = (scratch[2 * index], scratch[2 * index + 1]);
            mix[2 * index] += l * gain;
            mix[2 * index + 1] += r * gain;
            if self.released && index >= split {
                self.tail += 1;
                if l.abs().max(r.abs()) < SILENCE {
                    self.quiet += 1;
                } else {
                    self.quiet = 0;
                }
            }
        }
        playing
            && self.fade != Some(0)
            && self.quiet < self.quiet_limit
            && self.tail < self.tail_limit
    }
}

/// Mixes voices, starting each at its frame. Used on the audio thread by
/// [`Mixer`](crate::Mixer) and directly by offline rendering.
pub(crate) struct Engine {
    /// The next frame to render.
    frame: u64,
    voices: Vec<Voice>,
    /// Scheduled voices whose start frame hasn't come yet.
    pending: Vec<Voice>,
    scratch: Box<[f32]>,
}

impl Engine {
    pub(crate) fn new() -> Self {
        Self {
            frame: 0,
            voices: Vec::with_capacity(VOICE_SLOTS),
            pending: Vec::with_capacity(PENDING_SLOTS),
            scratch: vec![0.0; BLOCK * 2].into_boxed_slice(),
        }
    }

    /// The next frame to render: how many frames have been rendered.
    pub(crate) fn frame(&self) -> u64 {
        self.frame
    }

    /// Whether nothing is sounding or waiting to start.
    pub(crate) fn is_idle(&self) -> bool {
        self.voices.is_empty() && self.pending.is_empty()
    }

    /// Start `voice` at its start frame (straight away if that has
    /// passed). Gives it back if too many voices are already waiting.
    pub(crate) fn schedule(
        &mut self,
        voice: Voice,
    ) -> Result<(), Voice> {
        if self.pending.len() == PENDING_SLOTS {
            return Err(voice);
        }
        self.pending.push(voice);
        Ok(())
    }

    /// Fade out every sounding voice and drop every waiting one.
    pub(crate) fn stop_all(
        &mut self,
        discard: &mut dyn FnMut(Voice),
    ) {
        for voice in self.pending.drain(..) {
            discard(voice);
        }
        for voice in &mut self.voices {
            voice.fade_out();
        }
    }

    /// Render the next `out.len() / 2` frames into `out` (interleaved
    /// stereo, overwritten), clamped to ±1.
    pub(crate) fn render(
        &mut self,
        out: &mut [f32],
        discard: &mut dyn FnMut(Voice),
    ) {
        out.fill(0.0);
        for block in out.chunks_mut(BLOCK * 2) {
            let end = self.frame + (block.len() / 2) as u64;
            let mut index = 0;
            while index < self.pending.len() {
                if self.pending[index].start < end {
                    let voice = self.pending.swap_remove(index);
                    self.start_voice(voice, discard);
                } else {
                    index += 1;
                }
            }
            let mut index = 0;
            while index < self.voices.len() {
                if self.voices[index].mix(
                    self.frame,
                    block,
                    &mut self.scratch,
                ) {
                    index += 1;
                } else {
                    discard(self.voices.swap_remove(index));
                }
            }
            for sample in block.iter_mut() {
                *sample = sample.clamp(-1.0, 1.0);
            }
            self.frame = end;
        }
    }

    /// Add `voice`, fading out the oldest sounding voice if the limit is
    /// reached.
    fn start_voice(
        &mut self,
        voice: Voice,
        discard: &mut dyn FnMut(Voice),
    ) {
        let sounding =
            self.voices.iter().filter(|v| v.fade.is_none()).count();
        if sounding >= MAX_VOICES
            && let Some(oldest) = self
                .voices
                .iter_mut()
                .filter(|v| v.fade.is_none())
                .min_by_key(|v| v.start)
        {
            oldest.fade_out();
        }
        // Only reachable when voices are stolen faster than they fade:
        // cut the oldest outright.
        if self.voices.len() == VOICE_SLOTS
            && let Some(oldest) = (0..self.voices.len())
                .min_by_key(|&index| self.voices[index].start)
        {
            discard(self.voices.swap_remove(oldest));
        }
        self.voices.push(voice);
    }
}
