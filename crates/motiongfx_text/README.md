# MotionGfx text layout

Renderer-independent font shaping/layout for animated text. Parley resolves
glyphs, kerning, bidirectional runs, clusters, font metrics and line breaking.
There is no window, scene, GPU or second timeline here.

`FontSystem` owns shared font/layout contexts. It embeds Fira Sans regular,
bold, italic and bold italic. System font discovery is disabled for reproducible
output; applications register other font bytes explicitly with `register_font`.
Unknown family names fall back to Fira Sans. Embedded fonts do not cover every
script: applications must provide suitable fonts for additional character sets.

`LayoutStyle` contains layout inputs; `PreparedText` contains positioned glyph
runs, normalized variation coordinates and source cluster ranges. Font data is
Peniko `FontData`, and bounds use Kurbo, independently of Vello. Invalid numeric
styles produce empty output; `LayoutStyle::is_valid` exposes the check. Weight
is continuous and clamped to 1–1000 at layout; actual variation depends on the
font. The embedded faces are static, so changing weight selects faces rather
than providing a continuous variable-font morph.

`LayoutCache` retains one layout per owning text object, keyed by source,
layout style and font registration revision. Paint, opacity and transforms are
deliberately absent. Animated font size/spacing/weight can require reshaping;
animated scale can reuse the layout. No frame-time benchmark is claimed.
Cache validity is currently determined by comparing input keys on rendering;
there is no setter-driven invalidation protocol. This keeps direct writes,
bindings and backward sampling correct with the existing scene model. See
[text-caching.md](../../docs/text-caching.md) for the separate renderer paint-span
cache and the reasons for keeping the current checks until richer text effects.

`TextRange` validates UTF-8 byte boundaries and holds its exact source snapshot.
Selections expand to whole shaped clusters (including ligatures); bounds are
advance/line boxes, not ink-tight outlines. A changed source disables the range
until the original source is restored, including backward seeking. Automatic
range remapping, edit matching, glyph reveal/write-in and part transforms are
future work. This source/glyph distinction is the foundation for those features;
they are not implemented here.

Typst/LaTeX math typesetting is separate from plain text shaping. A future
typesetting adapter should preserve its positioned output and semantic parts,
rather than reshape it through Parley.

## Embedded font license

The Fira Sans files in `fonts/` are distributed under the SIL Open Font License
1.1; see [FiraSans-License.txt](fonts/FiraSans-License.txt). They originate from
the Fira Sans project (Mozilla/Telefonica), copied from the Bevy Feathers font
assets distributed in `bevy_feathers` 0.20.0. The font license is separate from
this crate's MIT/Apache-2.0 code license.
