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
    sequence.play(dot.position.x.to(100.0));
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

The initial text adapter still uses cached ASCII vector lettering, not font
shaping or Typst. The legacy public `lettering` module supports low-level host
examples. Rich font/layout integration can replace this renderer without
putting Vello resources into portable Text state.

Integration tests here compare Vello command output, exercise cache reuse and
verify authoring behavior against core MotionGfx, including backward seeking.
Portable geometry tests and a headless mixed-model test also live in
`motiongfx_2d`. The native preview host remains in `vello_winit_motiongfx`.
