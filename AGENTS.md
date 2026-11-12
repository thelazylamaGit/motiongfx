# Working on MotionGfx

Read [docs/architecture.md](docs/architecture.md) before changing the authoring,
signals, Vello, or preview architecture. It records the current implementation,
the reasons for it, limitations, and future possibilities. Crate READMEs explain
individual APIs; example source is the reference for working usage.

## Project direction

- Build a concise, strongly typed Rust motion-graphics authoring experience,
  inspired by Motion Canvas and Manim. Support procedural code, reusable
  components, and seeking through orchestrated animations.
- Keep MotionGfx's existing timeline, interpolation, baking, and sampling as the
  foundation. Do not introduce a competing timeline merely to shorten syntax.
- Keep window/GPU setup and playback controls out of animation descriptions.
  `motiongfx_authoring` is backend-independent; `vello_motiongfx` owns graphics;
  `vello_winit_motiongfx` owns the native preview host.
- Prefer explicit object creation and understandable animated properties.
  Short examples must still describe their objects, math, and animation; hiding
  a whole demonstration behind an example-specific binding is not an API gain.
- Improve the existing API before adding overlapping alternatives, a general UI
  framework, ECS, or a new reactive framework. Future ideas in the architecture
  note are not instructions to implement them.

## Authoring and correctness

- Standalone `signal(...)`, `.map(...)`, `computed(...)`, and `computed!` are the
  current reactive authoring API. Do not reintroduce `scene.signal`,
  `scene.computed`, `scene.value`, or `scene.derive` aliases without a reason.
- Prefer expression closures for simple math. For calculations with several
  statements, explicit `return` is welcome for readability. `computed!` supports
  both expression and block bodies.
- Borrow named reusable signal/computed descriptions in builders; inline
  temporaries can move. Moving a final use is also valid. Explain ownership
  clearly when discussing it with users new to Rust.
- Preserve arbitrary-time sampling, including backward seeks. Timeline baking
  resolves action targets; it does not precompute every procedural frame.
- Distinguish setup work, sampling work, and rendering work in performance
  claims. Native bindings are cached but not zero-cost. Measure before claiming
  improvements to frame time, compile time, or binary size.
- Preserve user edits. Commit only when requested. Do not redesign unrelated
  subsystems or migrate all examples merely because an optional macro exists.

## Verification and documentation

Run checks appropriate to the changed crates. For authoring/Vello/host changes:

```sh
cargo test -p motiongfx_authoring -p vello_motiongfx -p vello_winit_motiongfx -p vello_winit_example --all-targets
cargo test -p motiongfx_authoring -p vello_motiongfx -p vello_winit_motiongfx --doc
cargo clippy -p motiongfx_authoring -p vello_motiongfx -p vello_winit_motiongfx -p vello_winit_example --all-targets -- -D warnings
cargo fmt -p motiongfx_authoring -p vello_motiongfx -p vello_winit_motiongfx -p vello_winit_example -- --check
```

Put meaningful engine/API tests in crate tests. Avoid filling demonstration
files with assertions that merely repeat their authored sequence. Report
whether windows were visually checked; compilation is not visual validation.

Update the architecture note and relevant READMEs when a decision, public API,
or limitation changes. Keep this file short and link to details rather than
duplicating them. The workspace currently excludes `velyst_motiongfx` pending
compatible Bevy/Velyst dependencies; do not treat it as a working native Vello
text integration.
