//! Frames and the text that counts them: the timecode and the frame
//! readout in the transport bar.
//!
//! The animation itself is sampled at any time, not at frames; frames are
//! how the editor counts, labels and steps through it.

use std::time::Duration;

use vello_winit_motiongfx::PlaybackState;

use super::viewport::Viewport;

/// The frame rate the timecode, ruler and frame steps count in.
pub(crate) const FPS: u32 = 60;

/// One frame at [`FPS`].
pub(crate) const FRAME: Duration =
    Duration::from_nanos(1_000_000_000 / FPS as u64);

/// The frame `time` falls in. Rounded, so a seek to a frame's start
/// shows that frame despite float error.
pub(super) fn frame_of(time: Duration) -> u64 {
    (time.as_secs_f64() * f64::from(FPS)).round() as u64
}

/// `MM:SS:FF`, with frames at [`FPS`].
pub(super) fn timecode(time: Duration) -> String {
    let frames = frame_of(time);
    let fps = u64::from(FPS);
    let (minutes, rest) = (frames / (60 * fps), frames % (60 * fps));
    format!("{minutes:02}:{:02}:{:02}", rest / fps, rest % fps)
}

/// `frame 74 / 300  ·  60 fps`, led by the zoom while zoomed in.
pub(super) fn meta_text(
    state: PlaybackState,
    view: Viewport,
) -> String {
    let frames = format!(
        "frame {} / {}  ·  {FPS} fps",
        frame_of(state.time),
        frame_of(state.duration)
    );
    if view.is_zoomed() {
        format!(
            "{:.1}×  ·  {frames}",
            view.duration() / view.length()
        )
    } else {
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_counts_minutes_seconds_and_frames() {
        assert_eq!(timecode(Duration::ZERO), "00:00:00");
        assert_eq!(
            timecode(Duration::from_secs_f64(61.5)),
            "01:01:30"
        );
        // The last frame of a second, not the next second.
        assert_eq!(timecode(FRAME * 59), "00:00:59");
    }

    #[test]
    fn stepping_frames_lands_on_whole_frames() {
        assert_eq!(frame_of(FRAME * 7), 7);
        assert_eq!(frame_of(FRAME * 7 + Duration::from_nanos(3)), 7);
    }
}
