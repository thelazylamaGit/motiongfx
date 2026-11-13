# MotionGfx Interp

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/motiongfx#license)
[![Crates.io](https://img.shields.io/crates/v/motiongfx_interp.svg)](https://crates.io/crates/motiongfx_interp)
[![Downloads](https://img.shields.io/crates/d/motiongfx_interp.svg)](https://crates.io/crates/motiongfx_interp)
[![Docs](https://docs.rs/motiongfx_interp/badge.svg)](https://docs.rs/motiongfx_interp/latest/motiongfx_interp/)
[![CI](https://github.com/voxell-tech/motiongfx/workflows/CI/badge.svg)](https://github.com/voxell-tech/motiongfx/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**MotionGfx Interp** holds the easing curves and value interpolation
that [MotionGfx](https://crates.io/crates/motiongfx) is built on. It
lives in its own crate so projects that only need to move a number
from `a` to `b` can depend on it without pulling in the animation
runtime. Easing and interpolation are independent of each other.

## Easing

`ease` holds pure shaping functions of type `fn(f32) -> f32`. Each
takes a progress value in `0.0..=1.0` and returns a reshaped one in
the same range, anchored at `0.0` and `1.0`. `ease::linear` is the
identity. The other curves are grouped into one module per family, and
each module has `ease_in`, `ease_out`, and `ease_in_out`.

```rust
use motiongfx_interp::ease;

let p = ease::cubic::ease_in_out(0.25);
```

Families: `sine`, `quad`, `cubic`, `quart`, `quint`, `expo`, `circ`,
`back`, `elastic`. They follow the definitions from
[easings.net](https://easings.net/).

## Interpolation

`Interpolation` blends two values of the same type by a progress
value, usually the output of an easing function.

```rust
use motiongfx_interp::interpolation::Interpolation;

let x = f32::interp(&0.0, &10.0, 0.25); // 2.5
```

It is implemented for `f32`, `f64`, `i32`, `u32`, `u8`, `i64`, `u64`,
and `usize`. Integers add a rounded offset to the start value, so
large values keep their precision and the endpoints stay exact at
`t == 0.0` and `t == 1.0`.

The trait carries a marker type parameter:

```rust,ignore
pub trait Interpolation<M> { /* ... */ }
```

`M` exists only for the orphan rule. A downstream crate that wants to
interpolate a type it does not own can implement the trait against a
local marker instead. `impl_float_interpolation!` generates these
impls for any type with float arithmetic, and `impl_int_interpolation!`
reuses the built-in integer impls under a marker.

## Analytical springs

`spring::Spring` provides separate physical, visual and timed configuration types.
`prepare` validates settings once and returns a Copy analytical sampler:

```rust
use motiongfx_interp::spring::Spring;
use core::time::Duration;
let curve = Spring::visual(Duration::from_millis(500))
    .bounce(0.25).rest_delta(0.001).rest_speed(0.01).prepare()?;
let sample = curve.sample(Duration::from_millis(200));
assert!(sample.position.is_finite() && sample.velocity.is_finite());
let timed = Spring::timed(Duration::from_millis(500))
    .bounce(0.4).prepare()?;
assert_eq!(timed.duration(), Duration::from_millis(500));
# Ok::<(), motiongfx_interp::spring::SpringError>(())
```

Core actions accept settings directly; explicit `prepare` is for consumers such
as graph visualizers, not a mandatory `.build()`. Sampling evaluates closed-form
underdamped, critical or overdamped responses from absolute elapsed time.
Visual mode uses Motion's visual-duration calibration, including negative-bounce
compensation. The requested time controls pace; the settling tail may extend
beyond it. `arrival()` reports a separate first-95% diagnostic milestone.
`Spring::timed(duration).bounce(...)` instead fits the full settling tail into
exactly the requested time, using preparation-time frequency calibration.
`Spring` descriptions support bounded same-mode `Interpolation<()>` for
visualizers and `Display` for main-parameter readouts. Interpolating descriptions
does not modify already prepared spring actions; those retain fixed coefficients.
Rest tolerances use **normalized displacement and displacement/second**, not
pixels. This makes fixed analytical-curve scheduling independent of target
distance. The endpoint snaps the small residual to exact position 1, velocity 0.
See [defaults, validation, and timing semantics](../../docs/springs.md).

## `no_std` support

The `std` feature is on by default and uses the standard library's
float intrinsics. With it disabled, the same math goes through
[`libm`](https://crates.io/crates/libm) and the crate is `no_std`.

## Join the community!

You can join us on the [Voxell discord server](https://discord.gg/Mhnyp6VYEQ).

## License

`motiongfx_interp` is dual-licensed under either:

- MIT License ([LICENSE-MIT](../../LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer!
This dual-licensing approach is the de-facto standard in the Rust ecosystem and there are [very good reasons](https://github.com/bevyengine/bevy/issues/2373) to include both.
