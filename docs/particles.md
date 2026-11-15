# Deterministic particles (planned)

Status: agreed design, 2026-11-16. Not implemented. Build it for a concrete
example (confetti, rain), measuring sampling and rendering at 1k, 10k and 50k
particles.

## Pattern

Particles that can be scrubbed follow the established stateless pattern used
by shader particles, Remotion and After Effects expressions: there is no
particle framework, only a loop over particle indices, a seeded hash per
index, and everything computed from the effect's clock. Nothing is stored per
particle and nothing is spawned or removed, so seeking anywhere costs one
frame.

- **Randomness:** `Rng::new(seed, index)`, a stateless generator (splitmix64
  of both numbers). The seed is ordinary code.
- **Lifetime, spawning, disappearing:** ordinary code in the loop (`continue`
  once a particle is done, shrink it with its age).
- **Continuous effects (rain, snow):** loop the age with
  `age = (t + offset).rem_euclid(lifetime)`; `cycle = floor((t + offset) /
  lifetime)` mixed into the seed gives each respawn new random values.
- **Long streams:** loop over the live index range only (for a constant rate,
  `ceil((t - life) * rate) ..= floor(t * rate)`).

## API

```rust
let confetti = scene.add(Particles::new(|t, draw| {
    for i in 0..400 {
        let mut rng = Rng::new(SEED, i);
        let angle = rng.range(1.05..2.09);
        let speed = rng.range(500.0..900.0);
        let life = rng.range(2.5..3.5);
        if t >= life {
            continue;
        }
        let position = Point::new(
            speed * angle.cos() * t,
            speed * angle.sin() * t - 0.5 * GRAVITY * t * t,
        );
        let size = 8.0 * (1.0 - t / life);
        draw.shape(Rectangle::new(size, size / 2.0).position(position).fill(GOLD));
    }
}));
scene.compile([confetti.play_for(s(3.5))]);
```

- `Particles::new(|t, draw| ..)` is an ordinary object: position, rotation,
  scale and opacity setters move the whole effect.
- `play_for(d)` runs the effect's own clock linearly from 0 to `d` (an
  ordinary `clock().set(0.0)` then `clock().to(d)` change); `clock()` allows
  rewinds or custom easing. Nothing is drawn while the clock is 0. Playing
  again restarts the clock and replays the same particles; overlapping copies
  are separate effects (a function taking a seed).
- `draw.shape(..)` takes the existing shape descriptions with fixed values:
  circles, rectangles, triangles and chevrons. Lines, paths and text are not
  supported. A reactive input inside the closure panics at the caller's line.
- The closure runs only when the clock changes, into a reused shape buffer;
  one dynamic call per frame, with the loop and closure compiled together.
- The particle maths runs on the CPU; Vello rasterises on the GPU. The
  renderer culls shapes outside the viewport.

## Considered and rejected

- Emission types, lifetime/bounds settings, death-time searches and spawn
  closures: more API than the pattern needs.
- Runtime spawning, despawning or object pools: they break backward seeking.
- GPU-evaluated particles: needs shader code instead of a Rust closure and a
  pass outside Vello; reconsider only if measurements demand it.
