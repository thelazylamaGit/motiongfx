//! The editor's own icons, drawn in the 24-unit square and filled style of
//! `zgui_ui_icons`, which has no media-transport or speaker icons.
//!
//! Each outline is filled with the non-zero rule, so overlapping parts
//! wound the same way stay filled.

use zgui_ui_icons::prelude::IconData;

pub(crate) use zgui_ui_icons::set::chevron::CHEVRON_DOWN;

/// Start playing.
pub(crate) const PLAY: IconData =
    IconData::new("play", 24.0, "M8 5.5 L19 12 L8 18.5 Z");

/// Stop playing.
pub(crate) const PAUSE: IconData = IconData::new(
    "pause",
    24.0,
    "M6.5 5 H10 V19 H6.5 Z M14 5 H17.5 V19 H14 Z",
);

/// Go to the first frame: a bar, then a triangle pointing at it.
pub(crate) const TO_START: IconData = IconData::new(
    "to-start",
    24.0,
    "M5 5 H7.5 V19 H5 Z M19 5.5 L9 12 L19 18.5 Z",
);

/// Go to the last frame.
pub(crate) const TO_END: IconData = IconData::new(
    "to-end",
    24.0,
    "M16.5 5 H19 V19 H16.5 Z M5 5.5 L15 12 L5 18.5 Z",
);

/// Step back one frame: a chevron, then a bar.
pub(crate) const FRAME_BACK: IconData = IconData::new(
    "frame-back",
    24.0,
    "M15.5 6 L9.5 12 L15.5 18 L17 16.5 L12.5 12 L17 7.5 Z M7 6 H9 V18 H7 Z",
);

/// Step forward one frame.
pub(crate) const FRAME_FORWARD: IconData = IconData::new(
    "frame-forward",
    24.0,
    "M8.5 6 L14.5 12 L8.5 18 L7 16.5 L11.5 12 L7 7.5 Z M15 6 H17 V18 H15 Z",
);

/// A speaker's body, which every volume icon starts with.
macro_rules! speaker {
    () => {
        "M3 9 L7 9 L12 5 L12 19 L7 15 L3 15 Z "
    };
}

/// The small sound wave in front of the speaker: a band between two arcs
/// about the speaker's centre.
macro_rules! near_wave {
    () => {
        "M15.54 8.46 A5 5 0 0 1 15.54 15.54 L14.26 14.26 A3.2 3.2 0 0 0 14.26 9.74 Z "
    };
}

/// Quiet: the speaker and one wave.
pub(crate) const VOLUME_LOW: IconData = IconData::new(
    "volume-low",
    24.0,
    concat!(speaker!(), near_wave!()),
);

/// Loud: the speaker and two waves.
pub(crate) const VOLUME_HIGH: IconData = IconData::new(
    "volume-high",
    24.0,
    concat!(
        speaker!(),
        near_wave!(),
        "M18.01 5.99 A8.5 8.5 0 0 1 18.01 18.01 L16.74 16.74 A6.7 6.7 0 0 0 16.74 7.26 Z"
    ),
);

/// Muted: the speaker with a cross where the waves would be.
pub(crate) const VOLUME_MUTED: IconData = IconData::new(
    "volume-muted",
    24.0,
    concat!(
        speaker!(),
        "M20.23 14.79 L15.71 10.27 L16.77 9.21 L21.29 13.73 Z ",
        "M21.29 10.27 L16.77 14.79 L15.71 13.73 L20.23 9.21 Z"
    ),
);
