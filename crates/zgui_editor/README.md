# zgui_editor

An editor window for MotionGfx animations, built with
[zgui](https://github.com/zortax/zgui). The scene renders with Vello straight
into a zgui surface on zgui's own GPU device (no copy), above a timeline:
transport buttons (start, previous frame, play/pause, next frame, end), a
`MM:SS:FF` timecode and frame counter at 60 fps, a speed menu, a volume
control, and a ruler and lane with a playhead you drag to scrub.

`Window` has the same builder as `vello_winit_motiongfx::Window` (`new`,
`playback_mode`, `renderer`, and `audio` with the `audio` feature), so the
examples run in either host. Run them in the editor with the examples crate's
`zgui` feature:

```sh
cargo run -p vello_winit_example --example sorting --features zgui
```

Keys:

- Space and K play and pause. L plays forwards and J backwards; pressed
  again in the direction already playing, they shuttle faster (2x, 4x,
  8x). With K held, J and L step one frame.
- M mutes and unmutes.
- The left and right arrows step one frame (one second with Shift); Home and
  End jump to the ends.
- `=` and `-` zoom the timeline around the playhead, `\` fits the whole
  animation.

On the track, Ctrl+wheel (Cmd on macOS) or Alt+wheel zooms around the
pointer, down to four frames across, and the wheel scrolls through time.
Scrubbing samples the animation at the exact time under the pointer, not at
whole frames. While zoomed, a navigator bar under the lane shows and drags
the visible range, and the view turns a page when the playhead leaves it.
Dragging the playhead past either end of the track scrolls it, faster the
further past the pointer is. Ruler labels keep about 72 px apart at any zoom
or window width.

The speed menu sets how fast time runs (0.1x to 4x); the shuttle multiplies
it. The trigger turns amber whenever time isn't running at 1x. Sound follows
as a change of tempo: events come sooner or later and notes are held shorter
or longer, but keep their pitch.

The volume control appears only when the window plays audio
(`Window::audio`) and the scene has sound events. If no audio device opens,
it is shown disabled and says so. Its slider is perceptual: the level is
cubed into a gain, so equal steps sound roughly equal.

At narrow widths the transport bar leaves things out rather than wrapping:
the frame counter below 720 px, the total time below 560 px, and the start
and end buttons below 440 px.

## Look and code layout

`Window::color_scheme` picks the dark palette (the default), the light one,
or the desktop's (`ColorScheme::System`). The palettes live in `theme.rs` as
zgui_ui design tokens, so the library's menus, popovers, buttons and slider
share the timeline's greys and amber accent. Each part keeps its own style
sheet beside its code and names only tokens, and the track's canvases take
their colours from CSS, so nothing in Rust knows a colour.

| File | What it holds |
|---|---|
| `lib.rs` | `Window`, the builder |
| `editor.rs` | the editor view and its keys |
| `clock.rs` | playback: direction, speed, shuttle, and the sound it drives |
| `controls.rs` | the speed menu and the volume popover |
| `theme.rs` | the palettes and the window's rules |
| `icons.rs` | the transport and speaker icons |
| `preview.rs` | Vello rendering into the zgui surface |
| `timeline/` | the transport bar and track: `viewport`, `pointer`, `draw`, `ruler`, `timecode`, `style` |

## Tests

Interaction tests (`src/headless.rs`) run the real timeline, style sheets,
layout, event routing and frame clock over zgui's headless platform, with a
renderer that draws nothing and a clock the test steps frame by frame. They
press and drag on the track's laid-out box and check what the view and clock
did: scrubbing, scrolling past the edges, paging during playback, and the
shuttle. They need no window or graphics device, so they run with
`cargo test`; they don't check how anything looks, the Vello preview, or how
sound plays.

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
