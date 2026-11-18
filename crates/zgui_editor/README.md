# zgui_editor

An editor window for MotionGfx animations, built with
[zgui](https://github.com/zortax/zgui). The scene renders with Vello straight
into a zgui surface on zgui's own GPU device (no copy), above a timeline:
transport buttons (start, previous frame, play/pause, next frame, end), a
`MM:SS:FF` timecode and frame counter at 60 fps, and a ruler and lane with a
playhead you drag to scrub.

`Window` has the same builder as `vello_winit_motiongfx::Window` (`new`,
`playback_mode`, `renderer`, and `audio` with the `audio` feature), so the
examples run in either host. Run them in the editor with the examples crate's
`zgui` feature:

```sh
cargo run -p vello_winit_example --example sorting --features zgui
```

Keys: Space plays and pauses, the left and right arrows step one frame (one
second with Shift), Home and End jump to the ends.

## Dependency on the zgui fork

This crate uses a local checkout of zgui at `../../zgui` (next to the
`voxell` folder), on a fork branch that adds, beyond upstream:

- zgui PR #54, which moves zgui to wgpu 30 and Vello 0.11 so it shares one
  wgpu with MotionGfx;
- `SurfaceRenderer::extra_usage`, so Vello's compute pass can write the
  surface texture directly (`STORAGE_BINDING`);
- `SurfaceRenderer::needs_render`, so a seek while paused redraws the
  preview without rendering on every refresh;
- Vulkan before DX12 on Windows. wgpu's DX12 backend compiles shaders with
  FXC, which took about 5 s to create the Vello renderer here, against 20 ms
  on Vulkan with a warm driver cache.

Every workspace build resolves this path, so the checkout has to exist; switch
the workspace entries to a git dependency on the fork before sharing the repo.

## Size

zgui adds its own CSS engine (Stylo), layout, widgets and image decoders. For
the `sorting` example, measured on Windows: 13.2 MB → 33.7 MB with the default
release profile, and 8.0 MB → 16.4 MB with fat LTO, `opt-level = "s"`, one
codegen unit, `panic = "abort"` and stripped symbols. wgpu, Vello and Parley
are shared, not duplicated.
