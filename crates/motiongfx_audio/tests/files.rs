//! Sounds loaded from files: a WAV and a SoundFont, both written by the
//! tests themselves so no assets are needed.
#![cfg(any(feature = "symphonia", feature = "rustysynth"))]

use motiongfx::prelude::ms;
use motiongfx_audio::Score;
use motiongfx_authoring::{Animation, CompiledScene, Scene};
use std::path::PathBuf;
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

fn left(audio: &[f32]) -> Vec<f32> {
    audio.iter().step_by(2).copied().collect()
}

fn rms(signal: &[f32]) -> f32 {
    (signal.iter().map(|s| s * s).sum::<f32>() / signal.len() as f32)
        .sqrt()
}

/// A file in the system's temporary directory, removed when dropped.
struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str, bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "motiongfx_audio_{}_{name}",
            std::process::id()
        ));
        std::fs::write(&path, bytes)
            .expect("the temporary directory is writable");
        Self(path)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A RIFF chunk: its id, length and data, padded to an even length.
fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = id.to_vec();
    chunk.extend(u32::try_from(data.len()).unwrap().to_le_bytes());
    chunk.extend(data);
    if data.len() % 2 == 1 {
        chunk.push(0);
    }
    chunk
}

/// A RIFF or LIST chunk holding `form` and `chunks`.
fn list(id: &[u8; 4], form: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut data = form.to_vec();
    for c in chunks {
        data.extend(c);
    }
    chunk(id, &data)
}

/// `samples` as 16-bit little-endian PCM.
fn pcm16(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|&s| ((s * 32767.0).round() as i16).to_le_bytes())
        .collect()
}

#[cfg(feature = "symphonia")]
mod wav {
    use super::*;
    use motiongfx_audio::Sample;

    /// A mono 16-bit WAV of `samples` at `rate`.
    fn wav(rate: u32, samples: &[f32]) -> Vec<u8> {
        let mut format = Vec::new();
        format.extend(1u16.to_le_bytes()); // PCM
        format.extend(1u16.to_le_bytes()); // mono
        format.extend(rate.to_le_bytes());
        format.extend((rate * 2).to_le_bytes()); // bytes per second
        format.extend(2u16.to_le_bytes()); // bytes per frame
        format.extend(16u16.to_le_bytes()); // bits per sample
        list(
            b"RIFF",
            b"WAVE",
            &[
                chunk(b"fmt ", &format),
                chunk(b"data", &pcm16(samples)),
            ],
        )
    }

    #[test]
    fn a_wav_file_loads_and_plays_resampled() {
        // 100 ms of a constant half level at 24 kHz, played at 48 kHz.
        let file =
            TempFile::new("half.wav", &wav(24_000, &[0.5; 2400]));
        let sample = Sample::load(&file.0).expect("the WAV decodes");
        assert_eq!(sample.duration(), ms(100));

        let audio =
            left(&Score::new(&compile([sample.play()])).render(RATE));
        let level = audio[frames(ms(50))];
        assert!((level - 0.5).abs() < 1e-3, "{level}");
        // Rendering stops once nothing sounds: here, when the sample ends.
        assert!(audio.len() < frames(ms(110)), "ends after 100 ms");
    }

    #[test]
    fn a_missing_file_names_its_path() {
        let path =
            std::env::temp_dir().join("motiongfx_audio_missing.wav");
        let error =
            Sample::load(&path).expect_err("the file doesn't exist");
        assert_eq!(error.path(), path);
        assert!(
            error.to_string().contains("motiongfx_audio_missing.wav")
        );
    }
}

#[cfg(feature = "rustysynth")]
mod soundfont {
    use super::*;
    use motiongfx_audio::SoundFont;
    use motiongfx_authoring::all;

    /// A name field: `name` padded with zeros to 20 bytes.
    fn name(name: &str) -> Vec<u8> {
        let mut field = name.as_bytes().to_vec();
        field.resize(20, 0);
        field
    }

    /// A generator or bag record: two little-endian `u16`s.
    fn pair(a: u16, b: u16) -> Vec<u8> {
        [a.to_le_bytes(), b.to_le_bytes()].concat()
    }

    /// The smallest useful SoundFont: preset 0 in bank 0 plays one looped
    /// sine sample (440 Hz, recorded at A4) at full level.
    fn sine_font() -> Vec<u8> {
        let length = 4800u32; // exactly 44 cycles of 440 Hz at 48 kHz
        let mut samples: Vec<f32> = (0..length)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                0.5 * (std::f32::consts::TAU * 440.0 * t).sin()
            })
            .collect();
        // The format requires 46 zero samples after each sample.
        samples.extend([0.0; 46]);

        let preset = |label: &str, bag: u16| {
            [
                name(label),
                pair(0, 0),
                bag.to_le_bytes().to_vec(),
                vec![0; 12],
            ]
            .concat()
        };
        let instrument = |label: &str, bag: u16| {
            [name(label), bag.to_le_bytes().to_vec()].concat()
        };
        let sample_header = |label: &str, start: u32, end: u32| {
            [
                name(label),
                start.to_le_bytes().to_vec(),
                end.to_le_bytes().to_vec(),
                start.to_le_bytes().to_vec(), // loop start
                end.to_le_bytes().to_vec(),   // loop end
                RATE.to_le_bytes().to_vec(),
                vec![69, 0], // original pitch A4, no correction
                pair(0, 1),  // no link, mono
            ]
            .concat()
        };
        const INSTRUMENT: u16 = 41;
        const SAMPLE_MODES: u16 = 54;
        const SAMPLE_ID: u16 = 53;
        let terminal_sample = [name("EOS"), vec![0; 26]].concat();
        list(
            b"RIFF",
            b"sfbk",
            &[
                list(
                    b"LIST",
                    b"INFO",
                    &[
                        chunk(b"ifil", &pair(2, 1)),
                        chunk(b"isng", b"EMU8000\0"),
                        chunk(b"INAM", b"Sine\0\0"),
                    ],
                ),
                list(
                    b"LIST",
                    b"sdta",
                    &[chunk(b"smpl", &pcm16(&samples))],
                ),
                list(
                    b"LIST",
                    b"pdta",
                    &[
                        chunk(
                            b"phdr",
                            &[preset("Sine", 0), preset("EOP", 1)]
                                .concat(),
                        ),
                        chunk(
                            b"pbag",
                            &[pair(0, 0), pair(1, 0)].concat(),
                        ),
                        chunk(b"pmod", &[0; 10]),
                        chunk(
                            b"pgen",
                            &[pair(INSTRUMENT, 0), pair(0, 0)]
                                .concat(),
                        ),
                        chunk(
                            b"inst",
                            &[
                                instrument("Sine", 0),
                                instrument("EOI", 1),
                            ]
                            .concat(),
                        ),
                        chunk(
                            b"ibag",
                            &[pair(0, 0), pair(2, 0)].concat(),
                        ),
                        chunk(b"imod", &[0; 10]),
                        chunk(
                            b"igen",
                            &[
                                pair(SAMPLE_MODES, 1),
                                pair(SAMPLE_ID, 0),
                                pair(0, 0),
                            ]
                            .concat(),
                        ),
                        chunk(
                            b"shdr",
                            &[
                                sample_header("Sine", 0, length),
                                terminal_sample,
                            ]
                            .concat(),
                        ),
                    ],
                ),
            ],
        )
    }

    fn piano() -> (TempFile, SoundFont) {
        let file = TempFile::new("sine.sf2", &sine_font());
        let font =
            SoundFont::load(&file.0).expect("the SoundFont parses");
        (file, font)
    }

    #[test]
    fn a_soundfont_note_sounds_for_its_length() {
        let (_file, font) = piano();
        let piano = font.program(0);
        let audio = left(
            &Score::new(&compile([piano.note(69).over(ms(200))]))
                .render(RATE),
        );
        assert!(rms(&audio[frames(ms(50))..frames(ms(150))]) > 0.05);
        // Rendering stops once the released note has rung out.
        assert!(audio.len() < frames(ms(300)), "released");
    }

    #[test]
    fn releasing_a_soundfont_note_leaves_the_same_key_sounding() {
        let (_file, font) = piano();
        let piano = font.program(0);
        let later = || piano.note(69).over(ms(600)).delay(ms(100));
        let both = Score::new(&compile([all([
            piano.note(69).over(ms(300)),
            later(),
        ])]));
        let alone = Score::new(&compile([later()]));
        // One MIDI channel would end both notes at the first note-off.
        let window = frames(ms(450))..frames(ms(650));
        let both = rms(&left(&both.render(RATE))[window.clone()]);
        let alone = rms(&left(&alone.render(RATE))[window]);
        assert!(alone > 0.05, "the later note sounds ({alone})");
        assert!((both - alone).abs() < 1e-4, "{both} != {alone}");
    }

    #[test]
    #[should_panic(expected = "no preset for program 5")]
    fn a_missing_program_panics() {
        let (_file, font) = piano();
        let _ = font.program(5);
    }
}
