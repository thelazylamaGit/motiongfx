//! The transport bar's right-hand controls: the playing speed and the
//! volume.

use zgui::prelude::*;
use zgui::reactive::{RenderEffect, UnsyncCallback};
use zgui::view::ClassName;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;
use zgui_ui_icons::set::chevron::CHEVRON_DOWN;
use zgui_ui_primitives::{Align, Placement, Side};

use crate::editor::Clock;

/// The speeds the menu offers.
const SPEEDS: [f64; 7] = [0.1, 0.25, 0.5, 1.0, 1.5, 2.0, 4.0];

/// What the scene's sound can do, which decides whether there is a volume
/// control and whether it works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Only the `audio` feature can find a scene with sound.
#[cfg_attr(not(feature = "audio"), allow(dead_code))]
pub(crate) enum SoundStatus {
    /// No sound: the scene has none, or the window wasn't asked to play
    /// it. No control is shown.
    None,
    /// The scene has sound but no device would play it. The control is
    /// shown disabled, saying why.
    NoDevice,
    /// Sound plays.
    Playing,
}

/// The volume control's state, shared by the popover and the M key.
#[derive(Clone, Copy)]
pub(crate) struct Sound {
    status: SoundStatus,
    /// From 0 to 100, as the slider shows it.
    level: RwSignal<f64, LocalStorage>,
    muted: RwSignal<bool, LocalStorage>,
}

impl Sound {
    /// The volume control for sound in `status`, applied to `clock`.
    pub(crate) fn new(status: SoundStatus, clock: &Clock) -> Self {
        let sound = Self {
            status,
            level: RwSignal::new_local(100.0),
            muted: RwSignal::new_local(false),
        };
        if status == SoundStatus::Playing {
            let clock = clock.clone();
            let apply = RenderEffect::new(move |_| {
                clock.set_volume(sound.gain());
            });
            on_cleanup_local(move || drop(apply));
        }
        sound
    }

    /// The amplitude for the slider's level. Loudness follows amplitude
    /// roughly as a cube root, so cubing makes equal slider steps sound
    /// like equal steps.
    fn gain(self) -> f32 {
        if self.muted.get() {
            return 0.0;
        }
        (self.level.get() / 100.0).powi(3) as f32
    }

    /// Mute, or unmute back to the level it had.
    pub(crate) fn toggle_mute(self) {
        if self.status == SoundStatus::Playing {
            self.muted.update(|muted| *muted = !*muted);
        }
    }

    fn icon(self) -> IconData {
        let level = self.level.get();
        if self.muted.get() || level == 0.0 {
            VOLUME_MUTED
        } else if level < 50.0 {
            VOLUME_LOW
        } else {
            VOLUME_HIGH
        }
    }
}

/// The speed menu: a compact trigger showing the speed, amber whenever
/// time isn't running at 1x.
pub(crate) fn speed_menu(clock: Clock) -> impl IntoView {
    let speed = clock.speed;
    let chosen = RwSignal::new_local(value_of(1.0));
    let changed = Attrs::new()
        .class_toggle(ClassName::new("speed--changed"), move || {
            speed.get() != 1.0
        });
    let choose = UnsyncCallback::new(move |value: String| {
        clock.set_speed(value.parse().unwrap_or(1.0));
    });
    view! {
        DropdownMenu {
            DropdownMenuTrigger(
                variant = ButtonVariant::Ghost,
                size = ButtonSize::Sm,
                class = "speed",
                {..changed}
            ) {
                text(class = "speed__value") {{move || speed_label(speed.get())}}
                Icon(icon = CHEVRON_DOWN, size = IconSize::Sm)
            }
            DropdownMenuContent {
                MenuLabel {"Playback speed"}
                MenuRadioGroup(
                    value = chosen,
                    label = "Playback speed",
                    on_change = choose
                ) {
                    // The menu builds its items each time it opens.
                    {SPEEDS.into_iter().map(|choice| {
                        view! { MenuRadioItem(value = value_of(choice)) {{speed_label(choice)}} }
                            .into_view()
                    }).collect::<Vec<_>>()}
                }
                MenuSeparator()
                MenuLabel(class = "speed__hint") {"J / L shuttle, K plays or pauses"}
            }
        }
    }
}

/// The volume button and its popover, when the scene has sound.
pub(crate) fn volume_control(sound: Sound) -> impl IntoView {
    match sound.status {
        SoundStatus::None => None,
        SoundStatus::NoDevice => Some(AnyView::new(view! {
            Tooltip {
                TooltipTrigger {
                    Button(
                        variant = ButtonVariant::Ghost,
                        size = ButtonSize::IconSm,
                        disabled = true
                    ) {
                        Icon(icon = VOLUME_MUTED, label = "Volume")
                    }
                }
                TooltipContent {"No audio device; playing without sound"}
            }
        })),
        SoundStatus::Playing => {
            Some(AnyView::new(volume_popover(sound)))
        }
    }
}

fn volume_popover(sound: Sound) -> impl IntoView {
    let icon = Signal::derive_local(move || sound.icon());
    view! {
        Popover {
            PopoverTrigger(variant = ButtonVariant::Ghost, size = ButtonSize::IconSm) {
                Icon(icon = icon, label = "Volume")
            }
            PopoverContent(
                placement = Placement::new(Side::Top, Align::End),
                class = "volume"
            ) {
                row(class = "volume__row") {
                    Button(
                        variant = ButtonVariant::Ghost,
                        size = ButtonSize::IconSm,
                        on:click = move |_| sound.toggle_mute()
                    ) {
                        Icon(icon = icon, label = "Mute (M)")
                    }
                    Slider(
                        value = sound.level,
                        min = 0.0,
                        max = 100.0,
                        step = 5.0,
                        label = "Volume",
                        class = "volume__slider",
                        // Turning it up is a request to hear it.
                        on_change = UnsyncCallback::new(move |_: f64| sound.muted.set(false))
                    )
                    text(class = "volume__value") {{move || {
                        let level = if sound.muted.get() { 0.0 } else { sound.level.get() };
                        format!("{level:.0}%")
                    }}}
                }
            }
        }
    }
}

/// How the menu's radio items name a speed.
fn value_of(speed: f64) -> String {
    speed.to_string()
}

/// `1×`, `0.25×`, `1.5×`.
fn speed_label(speed: f64) -> String {
    let text = format!("{speed:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}×")
}

/// A speaker: the body, then whatever comes out of it.
macro_rules! speaker {
    () => {
        "M3 9 L7 9 L12 5 L12 19 L7 15 L3 15 Z "
    };
}

/// The small wave in front of the speaker, as a band between two arcs.
macro_rules! near_wave {
    () => {
        "M15.54 8.46 A5 5 0 0 1 15.54 15.54 L14.26 14.26 A3.2 3.2 0 0 0 14.26 9.74 Z "
    };
}

const VOLUME_LOW: IconData = IconData::new(
    "volume-low",
    24.0,
    concat!(speaker!(), near_wave!()),
);

const VOLUME_HIGH: IconData = IconData::new(
    "volume-high",
    24.0,
    concat!(
        speaker!(),
        near_wave!(),
        "M18.01 5.99 A8.5 8.5 0 0 1 18.01 18.01 L16.74 16.74 A6.7 6.7 0 0 0 16.74 7.26 Z"
    ),
);

/// The speaker with a cross where the waves would be: two bars wound the
/// same way, so where they overlap stays filled.
const VOLUME_MUTED: IconData = IconData::new(
    "volume-muted",
    24.0,
    concat!(
        speaker!(),
        "M20.23 14.79 L15.71 10.27 L16.77 9.21 L21.29 13.73 Z ",
        "M21.29 10.27 L16.77 14.79 L15.71 13.73 L20.23 9.21 Z"
    ),
);

/// How the controls look.
pub(crate) const SHEET: &str = zgui::css!(
    ".transport .speed {
        flex-shrink: 0;
        gap: 2px;
        padding: 0 6px 0 8px;
        font-variant-numeric: tabular-nums;
    }

    .speed__value {
        white-space: nowrap;
        min-width: 30px;
    }

    .transport .speed--changed {
        color: #f2b84a;
    }

    .speed__hint {
        font-weight: normal;
        opacity: 0.7;
    }

    .volume__row {
        align-items: center;
        gap: 10px;
    }

    .volume .volume__slider {
        width: 140px;
    }

    .volume__value {
        width: 36px;
        font-size: 12px;
        font-variant-numeric: tabular-nums;
        white-space: nowrap;
        text-align: right;
        color: #9aa1b1;
    }"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_read_without_trailing_zeros() {
        assert_eq!(speed_label(1.0), "1×");
        assert_eq!(speed_label(0.25), "0.25×");
        assert_eq!(speed_label(1.5), "1.5×");
        assert_eq!(speed_label(0.1 * 8.0), "0.8×");
    }

    #[test]
    fn every_menu_speed_parses_back() {
        for speed in SPEEDS {
            assert_eq!(value_of(speed).parse::<f64>(), Ok(speed));
        }
    }
}
