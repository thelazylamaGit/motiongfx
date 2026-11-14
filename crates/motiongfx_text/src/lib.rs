//! Font layout, immutable source selections and positioned glyphs, without a GPU.
//! Layout excludes paint and scene transforms so those can animate independently.
use kurbo::{Rect, Size};
use motiongfx_interp::interpolation::Interpolation;
pub use parley::FontStyle;
use parley::{FontContext, LayoutContext, LineHeight, StyleProperty};
pub use peniko::FontData;
use std::{fmt, ops::Range, sync::Arc};

/// Continuous weight; the resolved font determines its available range/faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontWeight(f64);
impl FontWeight {
    pub const NORMAL: Self = Self(400.0);
    pub const BOLD: Self = Self(700.0);
    pub const fn new(value: f64) -> Self {
        Self(value)
    }
    pub const fn value(self) -> f64 {
        self.0
    }
}
impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}
impl From<f64> for FontWeight {
    fn from(value: f64) -> Self {
        Self(value)
    }
}
impl Interpolation<()> for FontWeight {
    fn interp(a: &Self, b: &Self, t: f32) -> Self {
        Self(a.0 + (b.0 - a.0) * f64::from(t))
    }
}

/// Immutable selection into one exact source snapshot. Byte ranges are validated
/// at construction; after a content edit a selection is inactive until that
/// snapshot is restored, including on backward seeks. No stale offsets are used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRange {
    source: Arc<str>,
    range: Range<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionError {
    InvalidRange,
    MissingOccurrence,
}
impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRange => "text range must be ordered and lie on UTF-8 boundaries",
            Self::MissingOccurrence => "requested text occurrence does not exist",
        })
    }
}
impl std::error::Error for SelectionError {}
impl TextRange {
    pub fn new(
        source: impl Into<Arc<str>>,
        range: Range<usize>,
    ) -> Result<Self, SelectionError> {
        let source = source.into();
        if range.start > range.end
            || source.get(range.clone()).is_none()
        {
            return Err(SelectionError::InvalidRange);
        }
        Ok(Self { source, range })
    }
    pub fn find(
        source: impl Into<Arc<str>>,
        needle: &str,
        occurrence: usize,
    ) -> Result<Self, SelectionError> {
        let source = source.into();
        if needle.is_empty() {
            return Err(SelectionError::MissingOccurrence);
        }
        let (start, matched) = source
            .match_indices(needle)
            .nth(occurrence)
            .ok_or(SelectionError::MissingOccurrence)?;
        let end = start + matched.len();
        Self::new(source, start..end)
    }
    pub fn source(&self) -> &Arc<str> {
        &self.source
    }
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
    pub fn overlaps(
        &self,
        source: &str,
        range: &Range<usize>,
    ) -> bool {
        self.source.as_ref() == source
            && self.range.start < range.end
            && range.start < self.range.end
    }
}

/// Layout inputs only. Transform, fill, opacity and selection paint are absent.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutStyle {
    pub family: Arc<str>,
    pub size: f64,
    pub weight: FontWeight,
    pub style: FontStyle,
    /// Baseline spacing as a multiple of font size.
    pub line_height: f64,
    pub letter_spacing: f64,
    /// Request equal-width digits (OpenType `tnum`) so changing numbers do not
    /// shift neighbouring glyphs. Fonts without `tnum` keep their default digits.
    pub tabular_numbers: bool,
    /// Zero means unconstrained width.
    pub wrap_width: f64,
}
impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            family: "Fira Sans".into(),
            size: 24.0,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
            line_height: 1.3,
            letter_spacing: 0.0,
            tabular_numbers: false,
            wrap_width: 0.0,
        }
    }
}
impl LayoutStyle {
    /// Check numeric inputs before passing them to the font layout engine.
    pub fn is_valid(&self) -> bool {
        [
            self.size,
            self.weight.value(),
            self.line_height,
            self.letter_spacing,
            self.wrap_width,
        ]
        .into_iter()
        .all(|value| (value as f32).is_finite())
            && self.size > 0.0
            && self.line_height > 0.0
            && self.wrap_width >= 0.0
    }
}
/// Positioned in layout coordinates (y down); source ranges are cluster-atomic.
#[derive(Clone, Debug)]
pub struct Glyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub source: Range<usize>,
    pub bounds: Rect,
}
pub struct GlyphRun {
    pub font: FontData,
    pub font_size: f32,
    pub normalized_coords: Vec<i16>,
    pub glyphs: Vec<Glyph>,
}
/// Immutable output suitable for different renderers and future part effects.
pub struct PreparedText {
    pub source: Arc<str>,
    pub runs: Vec<GlyphRun>,
    pub size: Size,
}
impl PreparedText {
    /// Advance/line boxes, in layout coordinates. A partial ligature selection
    /// includes its full cluster; this is not an ink-tight outline bound.
    pub fn selection_bounds(
        &self,
        selection: &TextRange,
    ) -> Option<Rect> {
        self.runs
            .iter()
            .flat_map(|r| &r.glyphs)
            .filter(|g| selection.overlaps(&self.source, &g.source))
            .map(|g| g.bounds)
            .reduce(|a, b| a.union(b))
    }
}

/// Shared by an application's renderer. Embedded fonts make default output
/// reproducible; system font discovery is deliberately disabled. Register more
/// font files through `register_font`. No windowing or rendering dependency.
pub struct FontSystem {
    fonts: FontContext,
    layouts: LayoutContext<()>,
    revision: u64,
}
impl Default for FontSystem {
    fn default() -> Self {
        let mut system = Self {
            fonts: FontContext::new(),
            layouts: LayoutContext::new(),
            revision: 0,
        };
        for bytes in [
            include_bytes!("../fonts/FiraSans-Regular.ttf")
                .as_slice(),
            include_bytes!("../fonts/FiraSans-Bold.ttf").as_slice(),
            include_bytes!("../fonts/FiraSans-Italic.ttf").as_slice(),
            include_bytes!("../fonts/FiraSans-BoldItalic.ttf")
                .as_slice(),
        ] {
            assert!(
                system.register_font(bytes),
                "embedded font must be valid"
            );
        }
        system
    }
}
impl FontSystem {
    /// Returns false when the data contains no readable fonts. Registered data
    /// is retained by the font collection; layout keys include this revision.
    pub fn register_font(&mut self, bytes: &[u8]) -> bool {
        let added = self.fonts.collection.register_fonts(
            peniko::Blob::new(Arc::new(bytes.to_vec())),
            None,
        );
        if added.is_empty() {
            return false;
        }
        self.revision += 1;
        true
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Shape and wrap text. Unknown families fall back to embedded Fira Sans.
    /// Invalid numeric styles produce empty output; see `LayoutStyle::is_valid`.
    pub fn layout(
        &mut self,
        source: Arc<str>,
        style: &LayoutStyle,
    ) -> PreparedText {
        // Animated numeric properties can cross invalid ranges. Do not pass
        // invalid sizes to the shaper; an invalid style produces no glyphs.
        if !style.is_valid() {
            return PreparedText {
                source,
                runs: Vec::new(),
                size: Size::ZERO,
            };
        }
        let family = if self
            .fonts
            .collection
            .family_id(&style.family)
            .is_some()
        {
            style.family.as_ref()
        } else {
            "Fira Sans"
        };
        let mut builder = self.layouts.ranged_builder(
            &mut self.fonts,
            &source,
            1.0,
            false,
        );
        builder.push_default(parley::FontFamily::named(family));
        builder
            .push_default(StyleProperty::FontSize(style.size as f32));
        builder.push_default(parley::FontWeight::new(
            style.weight.value().clamp(1.0, 1000.0) as f32,
        ));
        builder.push_default(style.style);
        builder.push_default(LineHeight::FontSizeRelative(
            style.line_height as f32,
        ));
        builder.push_default(StyleProperty::LetterSpacing(
            style.letter_spacing as f32,
        ));
        if style.tabular_numbers {
            const TABULAR: [parley::FontFeature; 1] =
                [parley::FontFeature {
                    tag: parley::setting::Tag::new(b"tnum"),
                    value: 1,
                }];
            builder.push_default(StyleProperty::FontFeatures(
                (&TABULAR).into(),
            ));
        }
        let mut layout = builder.build(&source);
        layout.break_all_lines(
            (style.wrap_width > 0.0)
                .then_some(style.wrap_width as f32),
        );
        let size =
            Size::new(layout.width() as f64, layout.height() as f64);
        let mut runs = Vec::new();
        for line in layout.lines() {
            let mut x = line.metrics().offset;
            for run in line.runs() {
                let run_start = x;
                let clusters: Vec<_> = run.clusters().collect();
                let mut ranges: Vec<_> =
                    clusters.iter().map(|c| c.text_range()).collect();
                // Parley may split a ligature into logical caret components.
                // Keep all components associated with its drawable glyph(s).
                let mut start = 0;
                while start < clusters.len() {
                    let mut end = start + 1;
                    while end < clusters.len()
                        && clusters[end].is_ligature_continuation()
                    {
                        end += 1;
                    }
                    let range =
                        ranges[start].start..ranges[end - 1].end;
                    ranges[start..end].fill(range);
                    start = end;
                }
                let mut glyphs = Vec::new();
                for (visual_index, cluster) in
                    run.visual_clusters().enumerate()
                {
                    let range = ranges[run
                        .visual_to_logical(visual_index)
                        .unwrap()]
                    .clone();
                    let cluster_x = x;
                    let bounds = Rect::new(
                        cluster_x as f64,
                        (line.metrics().baseline
                            - line.metrics().ascent)
                            as f64,
                        (cluster_x
                            + cluster
                                .glyphs()
                                .map(|g| g.advance)
                                .sum::<f32>()
                                .max(cluster.advance()))
                            as f64,
                        (line.metrics().baseline
                            + line.metrics().descent)
                            as f64,
                    );
                    for glyph in cluster.glyphs() {
                        glyphs.push(Glyph {
                            id: glyph.id,
                            x: x + glyph.x,
                            y: line.metrics().baseline + glyph.y,
                            source: range.clone(),
                            bounds,
                        });
                        x += glyph.advance;
                    }
                }
                x = run_start + run.advance();
                runs.push(GlyphRun {
                    font: run.font().clone(),
                    font_size: run.font_size(),
                    normalized_coords: run
                        .normalized_coords()
                        .to_vec(),
                    glyphs,
                });
            }
        }
        PreparedText { source, runs, size }
    }
}

/// One cached layout per owning text object. Memory tracks live objects rather
/// than every string ever seen. Paint and transforms do not invalidate it.
#[derive(Default)]
pub struct LayoutCache {
    key: Option<(Arc<str>, LayoutStyle, u64)>,
    prepared: Option<PreparedText>,
    builds: usize,
}
impl LayoutCache {
    pub fn get(
        &mut self,
        fonts: &mut FontSystem,
        source: Arc<str>,
        style: LayoutStyle,
    ) -> &PreparedText {
        let key = (source, style, fonts.revision());
        if self.key.as_ref() != Some(&key) {
            self.prepared = Some(fonts.layout(key.0.clone(), &key.1));
            self.key = Some(key);
            self.builds += 1;
        }
        self.prepared.as_ref().unwrap()
    }
    pub fn builds(&self) -> usize {
        self.builds
    }
    /// The current layout, without performing another key comparison.
    pub fn prepared(&self) -> Option<&PreparedText> {
        self.prepared.as_ref()
    }
}
