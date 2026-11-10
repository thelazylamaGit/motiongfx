# Vello MotionGfx

Initial native graphics integration, currently implementing rectangles for
the `moving_square` comparison. `Drawing::rectangle` creates stored object
state and returns typed authoring properties. Handles belong to the drawing
that created them; do not mix handles between drawings.

`Drawing::animate` consumes the drawing, builds a MotionGfx timeline and
bakes its targets. `Animation::sample` uses the existing MotionGfx sampler;
`Animation::render` emits normal Vello scene commands. Object state is stored
once, with geometry transforms calculated when rendering. There is no
per-frame authoring interpreter. This does not remove ordinary MotionGfx
subject access, interpolation, or Vello command-recording costs.

Use centered logical canvas coordinates, y upward, and counterclockwise
radians. Rectangle scale changes geometry while keeping stroke width fixed.
Fill and stroke colors are independent. `smooth` reproduces Manim's default
normalized sigmoid easing for comparisons.

The implementation is intentionally small: circle/path components and a
general procedural-update hook are not implemented yet.
