# Vello MotionGfx

Vello renderer for [motiongfx_2d](../motiongfx_2d/README.md) sampled objects.
This crate owns Vello command emission and its path, text and rounded-triangle
caches. It does not own another Scene, timeline, signal store or object model.
Portable authoring types are re-exported for import compatibility; new animation
files should import them from `motiongfx_2d`.

```rust
use motiongfx::prelude::s;
use motiongfx_2d::prelude::*;
use vello_motiongfx::Renderer;

let mut scene = Scene::new();
let dot = scene.add(Circle::new(12.0));
let mut animation = scene.animate(|sequence| {
    sequence.play(dot.position_x().to(100.0));
});
let mut renderer = Renderer::default();
let mut frame = vello::Scene::new();
let size = Size::new(800.0, 600.0);
animation.sample(s(1));
renderer.render(&animation, &mut frame, size, size, 1.0);
```

`Renderer::render` reads a shared `Animation<M>` where `M: Model2d`.
`Renderer::render_graphics` accepts the sampled `Graphics` view directly.
These record commands; the caller resets/owns the Vello Scene and submits it
through Vello's GPU renderer. Viewport/canvas fitting, DPI and the portable
camera transform are applied here. The preview host handles submission.

Caches are initialized lazily on first rendering, outside cloned/baked subject
state, and updated only when their relevant inputs change. Source geometry and
visible path ranges, text content, and triangle size/angle/radius retain their
existing reuse rules. Sampling itself does not allocate these render caches.
A renderer can be reused across animations; caches reconcile counts and keys.
Rendering a frame still records the normal Vello commands.

Portable objects, styles, attachment semantics, camera and geometry live in
`motiongfx_2d`; generic preparation and sampling live in `motiongfx_authoring`.
Geometry/color interpolation remains supplied by `peniko_motiongfx`.
See [architecture](../../docs/architecture.md).

Text is shaped by [motiongfx_text](../motiongfx_text/README.md), then emitted as
Vello glyph runs. One renderer-owned `FontSystem` shares font resources and one
layout cache per text object retains only the current source/style. Font size,
family, weight, style, spacing and wrap width are layout inputs; fill, opacity,
selection paint and scene transforms are resolved afterwards. A separate paint-span
cache maps glyphs to selection IDs when layout or selection structure changes.
Paint animation reads current colors/opacity per span without repeating glyph
range matching; equal adjacent paints still batch into one Vello glyph run.
Selection range changes and reuse across different scenes invalidate the mapping;
position, scale and rotation do not. Reshaping happens
on relevant changes, not for every transform sample. The four embedded Fira Sans
faces provide deterministic default text without system font discovery.

Validity is currently checked by comparing cached inputs during rendering, not
by a new change-notification system. Public selection ranges are fixed during
preparation; comparing them remains a conservative safeguard for cache/renderer
reuse. All visible objects still record commands each render. Text caches avoid
shaping and range matching, not drawing. See [text caching](../../docs/text-caching.md)
for the exact checks and future considerations; this behaviour is unchanged.

Register custom font bytes with `renderer.fonts_mut().register_font(bytes)`
(check its boolean result), then select their actual family name in `Text`.
To use this renderer in a native preview, pass it to `Window::renderer(renderer)`.
The legacy `lettering` module remains only for immediate preview controls and
low-level demonstrations; authored Text has one font-backed rendering path.

Integration tests here compare Vello command output, exercise cache reuse and
verify authoring behavior against core MotionGfx, including backward seeking.
Portable geometry tests and a headless mixed-model test also live in
`motiongfx_2d`. The native preview host remains in `vello_winit_motiongfx`.
