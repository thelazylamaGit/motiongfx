# Text preparation and rendering caches

The current implementation caches reusable text preparation, not rendered frames.
Every visible text object still emits Vello commands on every render, just as
rectangles and circles do. There is no retained-layer renderer, partial redraw
system or UI invalidation framework here.

## The two caches

| Cache | Reusable output | Validity inputs |
| --- | --- | --- |
| `motiongfx_text::LayoutCache` | Parley-shaped glyph runs, positions, font data, source cluster ranges and bounds | Content, font family/size/weight/style, line height, letter spacing, wrap width and font-registration revision |
| Vello `TextLayout` paint spans | Contiguous glyph ranges associated with selection IDs | Layout build generation and the text's selection IDs/source ranges |

Shaping resolves glyphs, ligatures, advances and line breaks. This is more work
than describing a basic rectangle, and is unnecessary when text only moves or
changes color. The second cache avoids matching every glyph against every
selection again just to determine its paint. Neither cache stores a duplicate
string allocation per glyph; source snapshots use shared `Arc<str>` values.

## What happens during rendering

For each text object that reaches rendering (zero opacity/scale skips it):

1. Compare layout inputs with the cached key. Run Parley only on a cache miss.
   This compares property values; it does not parse letters to discover font
   settings or search for selection substrings. String equality can compare
   bytes when equally valued strings have different allocations.
2. Compare the cached layout generation and selection IDs/ranges. Rebuild paint
   spans only when those inputs differ. Rebuilding matches source ranges against
   glyph clusters once and records selection IDs, not frozen paint values.
3. Read the current fill/opacity for each span, combine adjacent equal paints,
   and emit Vello glyph commands with the current scene/object transform.

Position, scale and rotation changes rebuild neither cache. Fill and opacity
animation also rebuild neither cache. Font-size or wrapping changes can rebuild
layout even when no letters change; a new layout also rebuilds the paint spans.
These checks and normal command recording still have costs. No frame-time
benchmark or claim that every check is constant-time has been made.

## Fixed selections and current rationale

The public authoring API fixes selection ranges during preparation. It animates
their fill/opacity, not their ranges. Used queries resolve once during timeline
construction; substring search is not repeated during playback. Content changes
disable a selection until its exact source snapshot returns, including backward
seeking. A renderer can also be reused for a different animation with different
selections, even if text content and object indices happen to match.

The per-render range comparison is deliberately conservative and remains in
place for now. Comparing final sampled inputs works with timeline writes,
computed bindings and direct mutable callbacks without requiring all writers to
participate in a new revision/invalidation protocol. Changing only builder
setters would miss writes performed later during sampling.

Revisit the keys, prepared-selection identity and invalidation strategy when
dynamic text parts, morphing or edit matching are designed. Geometry-only part
motion could preserve source associations; changed glyph correspondence may
require rebuilding them. This note documents today's implementation, not a
commitment to a future reactive or retained-rendering architecture.

Implementation: [shared layout cache](../crates/motiongfx_text/src/lib.rs),
[Vello paint spans](../crates/vello_motiongfx/src/text_renderer.rs), and
[deferred selections](../crates/motiongfx_2d/src/text_selection.rs).
