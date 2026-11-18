//! The ruler: how far apart its ticks are at the current zoom, and the
//! labels on the major ones.

use zgui::prelude::*;
use zgui::view::CustomPropertyName;

use super::timecode::FPS;
use super::viewport::Viewport;

/// The room each ruler label is given, in CSS pixels.
const LABEL_GAP: f64 = 72.0;

/// The ruler's step between labelled (major) ticks and how many parts it
/// splits into, for `length` seconds across `width` CSS pixels: the finest
/// that gives each label [`LABEL_GAP`] of room, so the ruler reads the
/// same at every zoom and window size. Before the first layout, when the
/// width is still zero, it allows ten labels.
pub(super) fn ruler_step(length: f64, width: f64) -> (f64, u32) {
    const F: f64 = 1.0 / FPS as f64;
    // Frames first, then seconds and minutes, each split into parts that
    // land on round numbers.
    const STEPS: [(f64, u32); 16] = [
        (F, 1),
        (2.0 * F, 2),
        (5.0 * F, 5),
        (10.0 * F, 2),
        (15.0 * F, 3),
        (30.0 * F, 2),
        (1.0, 4),
        (2.0, 4),
        (5.0, 5),
        (10.0, 5),
        (15.0, 3),
        (30.0, 6),
        (60.0, 6),
        (120.0, 4),
        (300.0, 5),
        (600.0, 5),
    ];
    let most = if width > 0.0 {
        (width / LABEL_GAP).max(1.0)
    } else {
        10.0
    };
    STEPS
        .into_iter()
        .find(|(major, _)| length / major <= most)
        .unwrap_or((1200.0, 4))
}

/// The labels of the major ticks in `view`, across a track `width` CSS
/// pixels wide. Each is placed by the `--mg-at` property, a percentage of
/// the track the style sheet turns into `left`.
pub(super) fn labels(view: Viewport, width: f64) -> Vec<impl View> {
    let (major, _) = ruler_step(view.length(), width);
    let first = (view.start() / major - 1e-9).ceil() as i64;
    let last = (view.end() / major + 1e-9).floor() as i64;
    (first..=last)
        .filter_map(|index| {
            let at = index as f64 * major;
            let fraction = view.fraction_of(at);
            // The last label would hang off the track's end.
            (fraction < 0.98).then(|| {
                let place = Attrs::new().custom_property(
                    CustomPropertyName::new("mg-at"),
                    Some(format!("{:.4}%", fraction * 100.0)),
                );
                view! {
                    text(class = "tick__label", {..place}) {{ruler_label(at)}}
                }
                .into_view()
            })
        })
        .collect()
}

/// A ruler label: `3s`, `1m05s`, or with frames, `3s 15f` or `15f`.
fn ruler_label(seconds: f64) -> String {
    let frames = (seconds * f64::from(FPS)).round() as u64;
    let fps = u64::from(FPS);
    let (whole, frame) = (frames / fps, frames % fps);
    let base = if whole >= 60 {
        format!("{}m{:02}s", whole / 60, whole % 60)
    } else {
        format!("{whole}s")
    };
    match (whole, frame) {
        (_, 0) => base,
        (0, frame) => format!("{frame}f"),
        (_, frame) => format!("{base} {frame}f"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rulers_label_seconds_minutes_and_frames() {
        assert_eq!(ruler_label(3.0), "3s");
        assert_eq!(ruler_label(65.0), "1m05s");
        assert_eq!(ruler_label(2.5), "2s 30f");
        assert_eq!(ruler_label(0.25), "15f");
    }

    #[test]
    fn ruler_labels_get_room_whatever_the_width() {
        for width in [300.0, 860.0, 2000.0] {
            for length in [4.0 / 60.0, 0.5, 3.0, 10.0, 75.0, 900.0] {
                let (major, _) = ruler_step(length, width);
                assert!(
                    major / length * width >= LABEL_GAP,
                    "{length} s across {width} px"
                );
            }
        }
        // A narrow track labels less often than a wide one.
        assert!(
            ruler_step(10.0, 300.0).0 > ruler_step(10.0, 2000.0).0
        );
        // Before layout, about ten labels.
        let (major, _) = ruler_step(4.0 / 60.0, 0.0);
        assert!((major - 1.0 / 60.0).abs() < 1e-12);
    }
}
