//! Renderer-owned text caches. Paint and transforms are resolved after layout.
use crate::{Text, TextAlign, TextSelection};
use motiongfx_text::{FontSystem, LayoutCache, LayoutStyle};
use std::{ops::Range, sync::Arc};
use vello::kurbo::Affine;
use vello::peniko::{Color, Fill};

#[derive(Default)]
pub(crate) struct TextLayout {
    active_selections: Vec<usize>,
    pub(crate) fonts: LayoutCache,
}
fn anchor(align: TextAlign, width: f64) -> f64 {
    match align {
        TextAlign::Left => 0.0,
        TextAlign::Center => width / 2.0,
        TextAlign::Right => width,
    }
}
fn paint(
    text: &Text,
    range: &Range<usize>,
    selections: &[TextSelection],
    active: &[usize],
) -> Color {
    let mut color = text.fill;
    let mut opacity = text.opacity.clamp(0.0, 1.0) as f32;
    for &index in active {
        let selection = &selections[index];
        let selected = selection.range.range();
        if selected.start < range.end && range.start < selected.end {
            color = selection.fill;
            opacity = text.opacity.clamp(0.0, 1.0) as f32
                * selection.opacity.clamp(0.0, 1.0) as f32;
        }
    }
    color.with_alpha(color.components[3] * opacity)
}
impl TextLayout {
    pub(crate) fn render(
        &mut self,
        text: &Text,
        scene: &mut vello::Scene,
        screen: Affine,
        fonts: &mut Option<FontSystem>,
        selections: &[TextSelection],
    ) {
        if text.scale == 0.0 || text.opacity <= 0.0 {
            return;
        }
        self.active_selections.clear();
        for id in text.selections() {
            if let Some(selection) = selections.get(id.index())
                && (Arc::ptr_eq(
                    selection.range.source(),
                    &text.content,
                ) || selection.range.source().as_ref()
                    == text.content.as_ref())
            {
                self.active_selections.push(id.index());
            }
        }
        let transform = screen
            * Affine::translate((text.position.x, -text.position.y))
            * Affine::rotate(-text.rotation)
            * Affine::scale(text.scale);
        let layout = self.fonts.get(
            fonts.get_or_insert_with(FontSystem::default),
            text.content.clone(),
            LayoutStyle {
                family: text.font_family.clone(),
                size: text.font_size,
                weight: text.font_weight,
                style: text.font_style,
                line_height: text.line_height,
                letter_spacing: text.letter_spacing,
                wrap_width: text.wrap_width,
            },
        );
        let transform = transform
            * Affine::translate((
                -anchor(text.align, layout.size.width),
                0.0,
            ));
        for run in &layout.runs {
            // Batch adjacent glyphs with the same resolved paint. Selection
            // animation reads no font files and performs no shaping.
            let mut start = 0;
            while start < run.glyphs.len() {
                let color = paint(
                    text,
                    &run.glyphs[start].source,
                    selections,
                    &self.active_selections,
                );
                let mut end = start + 1;
                while end < run.glyphs.len()
                    && paint(
                        text,
                        &run.glyphs[end].source,
                        selections,
                        &self.active_selections,
                    ) == color
                {
                    end += 1;
                }
                if color.components[3] > 0.0 {
                    scene
                        .draw_glyphs(&run.font)
                        .font_size(run.font_size)
                        .normalized_coords(&run.normalized_coords)
                        .transform(transform)
                        .brush(color)
                        .draw(
                            Fill::NonZero,
                            run.glyphs[start..end].iter().map(|g| {
                                vello::Glyph {
                                    id: g.id,
                                    x: g.x,
                                    y: g.y,
                                }
                            }),
                        );
                }
                start = end;
            }
        }
    }
}
