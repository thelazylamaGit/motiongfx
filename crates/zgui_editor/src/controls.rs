//! The transport bar's right-hand controls: the playing speed and the
//! volume. Both are zgui_ui components (a dropdown menu and a popover);
//! their colours come from the editor's theme.

use zgui::prelude::*;
use zgui::reactive::{RenderEffect, UnsyncCallback};
use zgui::view::ClassName;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;
use zgui_ui_primitives::{Align, Placement, Side};

use crate::clock::Clock;
use crate::icons::{
    CHEVRON_DOWN, VOLUME_HIGH, VOLUME_LOW, VOLUME_MUTED,
};

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

/// The speed menu: a compact trigger showing the speed, in the accent
/// colour whenever time isn't running at 1x.
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

/// The speaker button and the popover it opens: mute, a level slider and
/// the level as a percentage.
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

/// How the controls look. Rules that adjust a zgui_ui component name a
/// parent too (`.transport .speed`): a component installs its own sheet
/// when it first appears, after this one, so an equally specific rule here
/// would lose to it.
pub(crate) const SHEET: &str = zgui::css!(
    "/* The speed menu's trigger: the speed, in the accent while it isn't
       1x. */

    .transport .speed {
        flex-shrink: 0;
        gap: 2px;
        padding: 0 6px 0 8px;
        font-variant-numeric: tabular-nums;
    }

    .speed__value {
        min-width: 30px;
        white-space: nowrap;
    }

    .transport .speed--changed {
        color: var(--zui-color-primary);
    }

    .speed__hint {
        font-weight: normal;
        color: var(--zui-color-muted-foreground);
    }

    /* The volume popover: mute, slider, level. The popover has a fixed
       width, so the slider takes up whatever the other two leave and the
       padding is the same on both sides. */

    .volume__row {
        width: 100%;
        align-items: center;
        gap: 10px;
    }

    .volume .volume__slider {
        flex-grow: 1;
        min-width: 0;
    }

    .volume__value {
        flex-shrink: 0;
        width: 36px;
        white-space: nowrap;
        text-align: right;
        font-size: 12px;
        font-variant-numeric: tabular-nums;
        color: var(--zui-color-muted-foreground);
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
