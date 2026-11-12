# MotionGfx

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/motiongfx#license)
[![Crates.io](https://img.shields.io/crates/v/motiongfx.svg)](https://crates.io/crates/motiongfx)
[![Downloads](https://img.shields.io/crates/d/motiongfx.svg)](https://crates.io/crates/motiongfx)
[![Docs](https://docs.rs/motiongfx/badge.svg)](https://docs.rs/motiongfx/latest/motiongfx/)
[![CI](https://github.com/voxell-tech/motiongfx/workflows/CI/badge.svg)](https://github.com/voxell-tech/motiongfx/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

[**MotionGfx**](https://motiongfx.voxell.dev) is a backend-agnostic motion graphics creation framework.
Free and open-source forever.

<p align="center">
  <img src="assets/hero.webp" alt="Ten bars grow upward one after another, then play back in reverse.">
</p>

## Features

- **Backend agnostic**: MotionGfx describes what changes and leaves
  drawing to the backend.
- **Just code**: Scenes are plain Rust, built with loops, functions
  and variables.
- **Relative actions**: Each action starts from the field's current
  value, so steps chain naturally.
- **Two-way playback**: Play at any speed, in either direction, or
  jump to any frame without re-simulating.
- **Batteries included**: Common easing and interpolation functions
  are built in.

## Example

```rust
// Move a ball up, then back down.
let track = [
    b.act(ball, path!(Ball.y), |_| 30.0)
        .with_ease(ease::quad::ease_out)
        .play(ms(600)),
    b.act(ball, path!(Ball.y), |_| 140.0)
        .with_ease(ease::quad::ease_in)
        .play(ms(600)),
]
.ord_chain()
.compile();
```

<p align="center">
  <img src="assets/example.webp" alt="A ball rises with an ease-out, then falls with an ease-in.">
</p>

## Where Next

- [Native authoring architecture and direction](docs/architecture.md): crate
  boundaries, design decisions, current limitations and future options.
- [`vello_winit_example`](examples/vello_winit_example/README.md): native Rust
  authoring examples with shared Vello/Winit preview and scrubbing controls.
- [Contributor and agent guidance](AGENTS.md): implementation conventions and
  relevant validation commands.
- [Website](https://motiongfx.voxell.dev): guides and live demos.
- [`motiongfx`](crates/motiongfx): the core crate, with a full
  walkthrough.
- [`motiongfx_authoring`](crates/motiongfx_authoring): shared typed scene,
  sequence, signal and computed authoring.
- [`motiongfx_2d`](crates/motiongfx_2d): portable 2D objects, styles and geometry.
- [`vello_motiongfx`](crates/vello_motiongfx): Vello rendering and caches.
- [`vello_winit_motiongfx`](crates/vello_winit_motiongfx): native preview host.
- [`bevy_motiongfx`](crates/bevy_motiongfx): the Bevy backend.
- [Moxie](https://github.com/voxell-tech/moxie): a visual editor for
  MotionGfx.

## Inspirations and Similar Projects

- [Motion Canvas](https://motioncanvas.io/)
- [Manim](https://www.manim.community/)

## License

`motiongfx` is dual-licensed under either:

- MIT License ([LICENSE-MIT](LICENSE-MIT) or
  [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer! This dual-licensing
approach is the de-facto standard in the Rust ecosystem and there are
[very good reasons](https://github.com/bevyengine/bevy/issues/2373) to
include both.
