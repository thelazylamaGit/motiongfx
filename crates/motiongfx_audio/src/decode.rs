//! Decoding sound files with symphonia.

use crate::LoadError;
use std::path::Path;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// Decode the whole file at `path` into interleaved stereo, returning its
/// sample rate and samples.
pub(crate) fn decode(
    path: &Path,
) -> Result<(u32, Vec<f32>), LoadError> {
    let decode_error = |error| LoadError::decode(path, error);
    let file = std::fs::File::open(path)
        .map_err(|error| LoadError::io(path, error))?;
    let stream =
        MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str())
    {
        hint.with_extension(extension);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(decode_error)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| LoadError::no_audio(path))?;
    let track_id = track.id;
    let parameters = track
        .codec_params
        .as_ref()
        .and_then(|parameters| parameters.audio())
        .ok_or_else(|| LoadError::no_audio(path))?
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(
            &parameters,
            &AudioDecoderOptions::default(),
        )
        .map_err(decode_error)?;

    let mut rate = 0;
    let mut stereo = Vec::new();
    let mut packet_samples = Vec::new();
    while let Some(packet) =
        format.next_packet().map_err(decode_error)?
    {
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A corrupt packet is skipped, as players do.
            Err(Error::DecodeError(_)) => continue,
            Err(error) => return Err(decode_error(error)),
        };
        let spec = decoded.spec();
        rate = spec.rate();
        let channels = spec.channels().count();
        if channels == 0 {
            continue;
        }
        packet_samples.resize(decoded.frames() * channels, 0.0);
        decoded.copy_to_slice_interleaved(&mut packet_samples);
        for frame in packet_samples.chunks_exact(channels) {
            let left = frame[0];
            let right = frame.get(1).copied().unwrap_or(left);
            stereo.extend([left, right]);
        }
    }
    if rate == 0 {
        return Err(LoadError::no_audio(path));
    }
    Ok((rate, stereo))
}
