# Shared authoring scene and renderer separation

Status: implemented 2026-11-13. [architecture.md](architecture.md) records the
current APIs, rationale and future options.

## Boundaries

| Responsibility | Owner |
| --- | --- |
| Actions, timelines, interpolation, baking, sampling | Existing `motiongfx` |
| Shared Scene/Animation lifecycle, registration/property preparation, signals/computations, native binding/update execution | `motiongfx_authoring` |
| 2D state/storage, shape builders/handles, geometry, styles, attachments, 2D camera semantics | `motiongfx_2d` |
| Font shaping/layout, glyph/source mappings and bounds | `motiongfx_text` |
| Vello command recording and renderer caches/resources | `vello_motiongfx` |
| Native window/surface/clock/controls | `vello_winit_motiongfx` |
| Serialized documents, named registries and compilation | Existing `motiongfx_scene`, distinct from live authoring |

No new state crate, timeline, ECS, 3D engine, scripting VM or compositor was
introduced. The existing Bevy integration retains its native component/asset
model; it has not yet been adapted to these portable 2D objects.

## Shared contracts

`Scene<M>` and `Animation<M>` own the one authoring/sampling implementation.
`World<M>` contains the chosen model, scene-local signal storage and sampled
time. Model-owned typed IDs permit adapters to implement the core
`SubjectSource<ModelId, State> for World<M>` across crate boundaries.
A composite model contains multiple stores and supplies their accessors.
No per-sample type map chooses the store.

`SceneObject<M>` registers objects/components. `ObjectDefinition<T, M>` holds
common descriptor/property registration. `Input<M, T>` supplies generic source
preparation. The 2D crate wraps these in local `Object<T, M>` / `Input<T, M>`
types to provide inherent shape setters and geometry conversions legally under
Rust's coherence/orphan rules. There is one binding evaluator and timeline.

Advanced adapters now target `Source<World<Graphics>>` rather than the old
concrete `Source<Graphics>` world. The ordinary standalone `Sequence<W>` and
`BindingPlan<W>` APIs remain available for integrations using their own worlds.

The 2D `Scene` / `Animation` convenience aliases use `Graphics`. A composite
model implements `Model2d` and uses `motiongfx_authoring::Scene<MyModels>`.
Its 2D objects can share signals, computed relationships and scheduled changes
with other models. Source types usually infer the model from `scene.add`;
a completely detached descriptor may need an explicit `Object<Circle>` type.

Shared callback execution uses `UpdateTargets<M>`. Safe borrowing of concrete
storage stays in model adapters. The optional `UpdateStorage`/`UpdateHandle`
contracts provide generic tuple/array scheduling; the current 2D collection
adapter targets `Graphics`. Composite callback targets require their own safe
borrowing adapter or specific `UpdateTargets` implementation. There is no
implicit mixed-store callback ordering or aliasing scheme.

2D setup/read conveniences are extension traits, imported through
`motiongfx_2d::prelude::*`: `Scene2dExt` and `Animation2dExt`. Ordinary lifecycle
methods remain inherent shared methods. Handles retain the pre-existing scene
identity limitation: do not mix handles from different scenes.

## Sampling and rendering

Finalization validates the native binding plan, bakes relative action targets,
and samples time zero. Baking does not precompute procedural frames. Sampling
still applies the existing core timeline, evaluates prepared native bindings,
then runs callbacks in registration order. Standalone signals, `.map`,
`computed`, `computed!`, partial styles and typed tips retain their semantics.

`vello_motiongfx::Renderer` consumes read-only sampled model state. It owns
path traces, text layouts and rounded-triangle caches, initialized on first
draw and updated by their existing input keys. These no longer live in model
subjects or shared finalization. Rendering still records normal Vello commands.
The native host samples the shared animation and then invokes that renderer.
The background remains a renderer clear/base color, independent of the camera.

`vello_motiongfx` temporarily re-exports portable types for import compatibility;
it contains no second Scene, object model, signal storage or evaluator. New
examples import `motiongfx_2d`. Existing low-level demos remain explicit host
and renderer extension examples.

This split adds static type boundaries and construction adapters. It adds no
new per-sample scheduler, scene-description traversal, graph sorting or model
registry lookup. Native bindings, callbacks, interpolation and renderer commands
retain their ordinary costs. No frame-time, compile-time or binary-size
improvement has been benchmarked or claimed.

## Validation

- Headless authoring tests compose two independent test model adapters with
  shared sources, one timeline, parallel actions, waits, callbacks, validation
  errors and backward seeks.
- A headless `motiongfx_2d` integration test combines actual Circle builders with
  a second model; a computed relationship crosses the stores in both directions
  on the same schedule, including backward seeking.
- Existing Vello tests retain core-equivalence, command-output comparisons,
  source/callback behavior, style overrides, geometry/cache and camera checks.
- Example builds and host playback tests cover the migrated native integration.
- Dependency trees verify authoring and the 2D model have no Vello/Winit/GPU
  dependency. Geometry/colors come directly from Kurbo/Peniko.

Run the affected tests, doctests, Clippy and formatting commands in AGENTS.md.
Headless checks are not visual window validation.

## Later mixed rendering

A future 3D model can share the same authoring lifecycle and core timeline.
Renderer composition remains separate work: transparent overlays, 2D textures
on 3D planes, depth-aware mesh rendering and custom GPU effects have different
requirements. The existing host renders Vello into a texture but is not a
multipass compositor. Share device/queue and define texture formats, alpha,
depth and pass ordering when that concrete integration is implemented.

Renderer replacement depends on feature support. Define capabilities, fallbacks
or errors rather than silently discarding unsupported effects. A rendered 2D
texture does not automatically gain per-object depth or lighting. Bevy and a
browser/Wasm host need real adapters and testing; these are not implied by the
crate split alone.
