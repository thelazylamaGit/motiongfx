//! Renderer-owned text caches. Paint and transforms are resolved after layout.
use crate::{Text, TextAlign, TextSelection};
use motiongfx_text::{FontSystem, LayoutCache, LayoutStyle};
use std::ops::Range;
use vello::kurbo::Affine;
use vello::peniko::{Color, Fill};

#[derive(Default)]
pub(crate) struct TextLayout {
    selection_keys: Vec<(usize, motiongfx_text::TextRange)>,
    spans: Vec<Vec<PaintSpan>>,
    mapped_layout: usize,
    #[cfg(test)]
    pub(crate) mappings: usize,
    pub(crate) fonts: LayoutCache,
}
fn anchor(align: TextAlign, width: f64) -> f64 {
    match align {
        TextAlign::Left => 0.0,
        TextAlign::Center => width / 2.0,
        TextAlign::Right => width,
    }
}
struct PaintSpan {
    glyphs: Range<usize>,
    selection: Option<usize>,
}
fn paint(
    text: &Text,
    selection: Option<usize>,
    selections: &[TextSelection],
) -> Color {
    let (color, opacity) = match selection {
        Some(id) => (
            selections[id].fill,
            selections[id].opacity.clamp(0.0, 1.0),
        ),
        None => (text.fill, 1.0),
    };
    color.with_alpha(
        color.components[3]
            * (text.opacity.clamp(0.0, 1.0) * opacity) as f32,
    )
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
        let transform = screen
            * Affine::translate((text.position.x, -text.position.y))
            * Affine::rotate(-text.rotation)
            * Affine::scale(text.scale);
        self.fonts.get(
            fonts.get_or_insert_with(FontSystem::default),
            text.content.clone(),
            LayoutStyle {
                family: text.font_family.clone(),
                size: text.font_size,
                weight: text.font_weight,
                style: text.font_style,
                line_height: text.line_height,
                letter_spacing: text.letter_spacing,
                tabular_numbers: text.tabular_numbers,
                wrap_width: text.wrap_width,
            },
        );

        let layout =
            self.fonts.prepared().expect("layout was just prepared");
        let selections_changed = self.selection_keys.len()
            != text.selections().len()
            || self.selection_keys.iter().zip(text.selections()).any(
                |((index, range), id)| {
                    *index != id.index()
                        || *range != selections[id.index()].range
                },
            );
        if self.mapped_layout != self.fonts.builds()
            || selections_changed
        {
            self.selection_keys.clear();
            self.selection_keys.extend(text.selections().iter().map(
                |id| {
                    (id.index(), selections[id.index()].range.clone())
                },
            ));
            self.spans.clear();
            self.spans.extend(layout.runs.iter().map(|run| {
                let mut spans: Vec<PaintSpan> = Vec::new();
                for (index, glyph) in run.glyphs.iter().enumerate() {
                    let selection = self
                        .selection_keys
                        .iter()
                        .rev()
                        .find(|(_, range)| {
                            range.overlaps(
                                &layout.source,
                                &glyph.source,
                            )
                        })
                        .map(|(id, _)| *id);
                    if let Some(last) = spans.last_mut()
                        && last.selection == selection
                    {
                        last.glyphs.end = index + 1;
                    } else {
                        spans.push(PaintSpan {
                            glyphs: index..index + 1,
                            selection,
                        });
                    }
                }
                spans
            }));
            self.mapped_layout = self.fonts.builds();
            #[cfg(test)]
            {
                self.mappings += 1;
            }
        }
        let transform = transform
            * Affine::translate((
                -anchor(text.align, layout.size.width),
                0.0,
            ));

        for (run, spans) in layout.runs.iter().zip(&self.spans) {
            // Resolve changing paint per span, without repeating source-range
            // matching. Equal adjacent colors still share one Vello glyph run.
            let mut start = 0;
            while start < spans.len() {
                let color =
                    paint(text, spans[start].selection, selections);
                let mut end = start + 1;
                while end < spans.len()
                    && paint(text, spans[end].selection, selections)
                        == color
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
                            run.glyphs[spans[start].glyphs.start
                                ..spans[end - 1].glyphs.end]
                                .iter()
                                .map(|g| vello::Glyph {
                                    id: g.id,
                                    x: g.x,
                                    y: g.y,
                                }),
                        );
                }
                start = end;
            }
        }
    }
}
