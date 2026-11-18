//! The editor's colours: one palette per colour scheme, and the rules for
//! the window as a whole.
//!
//! The palettes are zgui_ui's design tokens (`--zui-color-*`) set to the
//! editor's colours, so the library's buttons, menus, popovers and sliders
//! and the editor's own timeline all draw from the same few values. Each
//! part of the editor keeps its own style sheet beside its code
//! (`timeline::style`, `controls`), and every rule in them names a token
//! rather than a colour; the timeline's canvases take their colour from
//! CSS too. So switching schemes changes everything at once and needs no
//! Rust, and a new part matches by using the same tokens.
//!
//! The palette is a neutral blue-grey with one accent, amber: the
//! playhead, the played range, a changed speed, a slider's fill and the
//! focus ring. Surfaces step up in lightness (dark) or down (light) from
//! the window to the timeline to anything floating over it.

use zgui::component;
use zgui::prelude::*;
use zgui_ui_tokens::prelude::*;

/// The dark scheme, which suits a preview: the picture is the brightest
/// thing on screen.
const DARK: &str = "
    --zui-color-background: #111318;
    --zui-color-foreground: #e8ecf4;
    --zui-color-card: #16181d;
    --zui-color-card-foreground: #e8ecf4;
    --zui-color-popover: #1c1f26;
    --zui-color-popover-foreground: #e8ecf4;
    --zui-color-muted: #1f2229;
    --zui-color-muted-foreground: #7d8597;
    --zui-color-accent: #272b34;
    --zui-color-accent-foreground: #f1f3f7;
    --zui-color-secondary: #23262e;
    --zui-color-secondary-foreground: #f1f3f7;
    --zui-color-border: #272b34;
    --zui-color-input: #2e333d;
    --zui-color-primary: #f2b84a;
    --zui-color-primary-foreground: #1d1405;
    --zui-color-ring: #f2b84a;
    --zui-color-tooltip: #2a2e38;
    --zui-color-tooltip-foreground: #e8ecf4;
    --zui-color-tooltip-border: #363b47;
";

/// The light scheme. The amber is darker here so that it still reads
/// against pale surfaces.
const LIGHT: &str = "
    --zui-color-background: #e9ecf1;
    --zui-color-foreground: #1b1e25;
    --zui-color-card: #f6f7f9;
    --zui-color-card-foreground: #1b1e25;
    --zui-color-popover: #ffffff;
    --zui-color-popover-foreground: #1b1e25;
    --zui-color-muted: #e3e7ee;
    --zui-color-muted-foreground: #5f6778;
    --zui-color-accent: #e3e7ee;
    --zui-color-accent-foreground: #1b1e25;
    --zui-color-secondary: #e1e5ec;
    --zui-color-secondary-foreground: #1b1e25;
    --zui-color-border: #d5dae3;
    --zui-color-input: #cdd3dd;
    --zui-color-primary: #b97a12;
    --zui-color-primary-foreground: #ffffff;
    --zui-color-ring: #b97a12;
    --zui-color-tooltip: #1b1e25;
    --zui-color-tooltip-foreground: #f6f7f9;
    --zui-color-tooltip-border: #1b1e25;
";

/// The editor's palettes around `children`, in `scheme`.
///
/// It is the outermost theme in the window, so it declares on `:root` and
/// also reaches menus and popovers, which open on an overlay outside this
/// subtree.
#[component]
pub(crate) fn EditorTheme(
    /// Light, dark, or whichever the desktop uses.
    scheme: ColorScheme,
    /// The editor.
    children: Children,
) -> impl IntoView {
    view! {
        ThemeProvider(
            scheme = RwSignal::new_local(scheme),
            light = Signal::derive_local(|| Theme::light().with_css(LIGHT)),
            dark = Signal::derive_local(|| Theme::dark().with_css(DARK))
        ) {
            {children.into_view_once()}
        }
    }
}

/// The rules for the window as a whole: its colours and type, and the
/// preview filling what the timeline leaves. Each part of the editor keeps
/// its own rules beside its code, written in the same tokens.
pub(crate) const SHEET: &str = zgui::css!(
    ":root {
        background-color: var(--zui-color-background);
        color: var(--zui-color-foreground);
        font-family: var(--zui-type-family-sans);
    }

    .editor {
        width: 100%;
        height: 100%;
    }

    .preview {
        flex-grow: 1;
        min-height: 0;
    }"
);

#[cfg(test)]
mod tests {
    use super::*;

    /// A token zgui doesn't know is dropped without a word, so check that
    /// every declaration in each palette reaches the sheet the theme
    /// writes.
    #[test]
    fn every_palette_declaration_is_taken_up() {
        for (palette, scheme) in
            [(DARK, ColorScheme::Dark), (LIGHT, ColorScheme::Light)]
        {
            let light = Theme::light().with_css(LIGHT);
            let dark = Theme::dark().with_css(DARK);
            let sheet = theme_sheet(":root", &light, &dark, scheme)
                .replace(char::is_whitespace, "");
            for declaration in palette.split(';') {
                let declaration =
                    declaration.replace(char::is_whitespace, "");
                if !declaration.is_empty() {
                    assert!(
                        sheet.contains(&declaration),
                        "{declaration} is missing from the theme"
                    );
                }
            }
        }
    }
}
