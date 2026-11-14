# Working on MotionGfx

Read [docs/architecture.md](docs/architecture.md) before changing the authoring,
signals, Vello, or preview architecture, and
[docs/authoring-redesign.md](docs/authoring-redesign.md) for how animations are
represented and compiled. Crate READMEs explain individual APIs; example source
is the reference for working usage.

## Project direction

- Build a concise, strongly typed Rust motion-graphics authoring experience,
  inspired by Motion Canvas and Manim. Support procedural code, reusable
  components, and seeking through orchestrated animations.
- Animations are values (`Animation`), combined with `chain`/`all`/`flow`/
  `wait`/`marker` and passed to `scene.compile(..)`, which lowers them to
  MotionGfx's existing timeline. Keep core's timeline, interpolation, baking
  and sampling as the foundation; don't add a competing timeline or evaluator.
- Keep window/GPU setup and playback controls out of animation descriptions.
  `motiongfx_authoring` owns shared scene/sampling; `motiongfx_2d` owns portable
  objects/geometry/styles; `motiongfx_text` owns font shaping/layout;
  `vello_motiongfx` owns rendering/caches; `vello_winit_motiongfx` owns the
  native preview host. See [the implemented separation](docs/backend-separation.md).
- Prefer explicit object creation and understandable animated properties.
  Short examples must still describe their objects, math, and animation; hiding
  a whole demonstration behind an example-specific binding is not an API gain.
- Keep the API small. Improve the existing API before adding overlapping
  alternatives, a general UI framework, ECS, or a new reactive framework.
  Remove what nothing uses. Future ideas in the docs are not instructions to
  implement them.
- Only allocate what a case needs; check type sizes and per-item allocations
  when designing data structures.

## Authoring and correctness

- Reactive inputs are standalone `signal(..)`, `.map(..)`, `computed(..)` and
  `computed!`. Prefer expression closures for simple math; explicit `return`
  is welcome in multi-statement calculations.
- Borrow named reusable signal/computed descriptions in builders; inline
  temporaries can move. Explain ownership clearly when discussing it with
  users new to Rust.
- Preserve arbitrary-time sampling, including backward seeks. Timeline baking
  resolves action targets; it does not precompute every procedural frame.
- Report authoring mistakes with the user's file and line: `#[track_caller]`
  panics for misuse visible while building, collected `compile` errors for
  problems found while compiling.
- Measure before claiming a performance change (setup, sampling and rendering
  are separate costs). Bench noise on this machine reaches ~30%, so A/B
  against a saved Criterion baseline and re-run surprising results.
- Preserve user edits. When work is finished and verified, commit it in
  focused commits on the current branch; don't push unless asked.

## Documentation

Follow the Rust API guidelines for doc comments:

- Start with one sentence saying what the item does or is, in the reader's
  terms. Don't restate the name, and don't list what it doesn't do.
- Document every public item. Add `# Panics` and `# Errors` sections where
  they apply, and a short `# Examples` block on main entry points. The
  authoring, 2D, Vello and host crates set `#![warn(missing_docs)]`, so
  clippy with `-D warnings` fails on an undocumented public item.
- Comments inside code explain *why*, not what the next line does.
- Performance notes, measurements and design history belong in `docs/`, not in
  doc comments.

Update the architecture note and relevant READMEs when a decision, public API,
or limitation changes. Keep this file short and link to details rather than
duplicating them.

## Verification

Build and test only the crates below. Never run workspace-wide commands
(`--workspace`), which build `bevy_motiongfx` and other Bevy crates that are
slow and outside this work.

```sh
P="-p motiongfx_text -p motiongfx_authoring -p motiongfx_2d -p vello_motiongfx -p vello_winit_motiongfx -p vello_winit_example"
cargo test $P --all-targets
cargo test -p motiongfx_text -p motiongfx_authoring -p motiongfx_2d -p vello_motiongfx -p vello_winit_motiongfx --doc
cargo clippy $P --all-targets -- -D warnings
cargo fmt $P -- --check
```

When core crates change (`motiongfx`, `motiongfx_interp`, `peniko_motiongfx`),
also test them, run `cargo check -p motiongfx --no-default-features` (no-std),
and clippy core with `--features diagnostics`.

Put meaningful engine/API tests in crate tests. Every example has a headless
build-and-sample test; avoid adding assertions that merely repeat an example's
authored sequence. Report whether windows were visually checked; compilation
is not visual validation.

The workspace excludes `velyst_motiongfx` pending compatible Bevy/Velyst
dependencies; do not treat it as a working native Vello text integration.
