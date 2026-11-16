//! [`Renderer`]: draws a sampled 2D scene into a `vello::Scene`.

use crate::triangle_geometry::CachedTriangle;
use crate::{path_renderer, text_renderer};
use motiongfx_2d::{
    Canvas, Cap, Chevron, Circle, Color, DrawObject, EndpointTip,
    Geometry, Join, Line, Model2d, Point, Rect, Rectangle, Size,
    Triangle,
};
use vello::kurbo::{
    Affine, Circle as CircleShape, Line as LineShape, PathEl, Stroke,
};
use vello::peniko::Fill;
trait RenderRectangle {
    fn render(&self, scene: &mut vello::Scene, screen: Affine);
}
impl RenderRectangle for Rectangle {
    fn render(&self, scene: &mut vello::Scene, screen: Affine) {
        let rectangle = self;
        let opacity = rectangle.opacity.clamp(0.0, 1.0) as f32;
        if opacity == 0.0 {
            return;
        }
        let xf = screen * self.transform();
        let half_w =
            rectangle.size.width * rectangle.transform.scale / 2.0;
        let half_h =
            rectangle.size.height * rectangle.transform.scale / 2.0;
        let shape = Rect::new(-half_w, -half_h, half_w, half_h)
            .to_rounded_rect(
                rectangle.radius * rectangle.transform.scale.abs(),
            );
        let fill = rectangle.fill;
        scene.fill(
            Fill::NonZero,
            xf,
            fill.with_alpha(fill.components[3] * opacity),
            None,
            &shape,
        );
        if rectangle.stroke_width > 0.0 {
            let stroke = rectangle.stroke;
            scene.stroke(
                &Stroke::new(rectangle.stroke_width)
                    .with_caps(rectangle.line_cap)
                    .with_join(rectangle.line_join),
                xf,
                stroke.with_alpha(stroke.components[3] * opacity),
                None,
                &shape,
            );
        }
    }
}
pub(crate) trait RenderCircle {
    fn render(
        &self,
        scene: &mut vello::Scene,
        parent_transform: Affine,
        inherited_opacity: f32,
    );
}
impl RenderCircle for Circle {
    fn render(
        &self,
        scene: &mut vello::Scene,
        parent_transform: Affine,
        inherited_opacity: f32,
    ) {
        let opacity =
            self.opacity.clamp(0.0, 1.0) as f32 * inherited_opacity;
        let transform = parent_transform * self.transform();
        if opacity == 0.0
            || self.radius <= 0.0
            || transform.determinant() == 0.0
        {
            return;
        }
        let shape = CircleShape::new(Point::ZERO, self.radius);
        if self.fill.components[3] > 0.0 {
            scene.fill(
                Fill::NonZero,
                transform,
                self.fill
                    .with_alpha(self.fill.components[3] * opacity),
                None,
                &shape,
            );
        }
        if self.stroke_width > 0.0 && self.stroke.components[3] > 0.0
        {
            scene.stroke(
                &Stroke::new(self.stroke_width),
                transform,
                self.stroke
                    .with_alpha(self.stroke.components[3] * opacity),
                None,
                &shape,
            );
        }
    }
}
pub(crate) trait RenderLine {
    fn render(
        &self,
        scene: &mut vello::Scene,
        screen: Affine,
        graphics: &Canvas,
        triangle_geometry: &[CachedTriangle],
    );
    fn draw_shaft(
        &self,
        scene: &mut vello::Scene,
        screen: Affine,
        start: Point,
        end: Point,
        opacity: f32,
    );
}
impl RenderLine for Line {
    fn render(
        &self,
        scene: &mut vello::Scene,
        screen: Affine,
        graphics: &Canvas,
        triangle_geometry: &[CachedTriangle],
    ) {
        let opacity = self.opacity.clamp(0.0, 1.0) as f32;
        if opacity <= 0.0 || self.reveal <= 0.0 {
            return;
        }
        let Some(layout) = self.layout(graphics, |id| {
            triangle_geometry[id.index()].geometry
        }) else {
            return;
        };
        if let Some((start, end)) = layout.shaft {
            self.draw_shaft(scene, screen, start, end, opacity);
        }
        for placement in layout.tips.into_iter().flatten() {
            let frame = screen
                * Affine::translate((
                    placement.position.x,
                    -placement.position.y,
                ))
                * Affine::rotate(-placement.rotation)
                * Affine::scale(placement.scale);
            placement.tip.render(
                graphics,
                triangle_geometry,
                scene,
                frame,
                opacity,
            );
        }
    }
    fn draw_shaft(
        &self,
        scene: &mut vello::Scene,
        screen: Affine,
        start: Point,
        end: Point,
        opacity: f32,
    ) {
        if self.stroke_width > 0.0 && self.stroke.components[3] > 0.0
        {
            let color = self
                .stroke
                .with_alpha(self.stroke.components[3] * opacity);
            let mut stroke = Stroke::new(self.stroke_width)
                .with_caps(self.line_cap)
                .with_join(self.line_join);
            if !self.line_dash.is_empty() {
                assert!(
                    self.dash_offset.is_finite()
                        && self
                            .line_dash
                            .iter()
                            .all(|d| d.is_finite() && *d >= 0.0)
                        && self.line_dash.iter().sum::<f64>() > 0.0,
                    "line dash lengths must be finite, nonnegative and have a positive period"
                );
                stroke = stroke.with_dashes(
                    self.dash_offset,
                    self.line_dash.iter(),
                );
            }
            scene.stroke(
                &stroke,
                screen,
                color,
                None,
                &LineShape::new((start.x, -start.y), (end.x, -end.y)),
            );
        }
    }
}
trait RenderTip {
    fn render(
        self,
        graphics: &Canvas,
        triangle_geometry: &[CachedTriangle],
        scene: &mut vello::Scene,
        transform: Affine,
        inherited_opacity: f32,
    );
}
impl RenderTip for EndpointTip {
    fn render(
        self,
        graphics: &Canvas,
        triangle_geometry: &[CachedTriangle],
        scene: &mut vello::Scene,
        transform: Affine,
        inherited_opacity: f32,
    ) {
        match self {
            Self::Circle(id) => graphics.circles()[id.index()]
                .render(scene, transform, inherited_opacity),
            Self::Triangle(id) => graphics.triangles()[id.index()]
                .render_geometry(
                    scene,
                    transform,
                    inherited_opacity,
                    triangle_geometry[id.index()].geometry,
                ),
            Self::Chevron(id) => graphics.chevrons()[id.index()]
                .render_geometry(
                    scene,
                    transform,
                    inherited_opacity,
                    graphics.chevrons()[id.index()].geometry(),
                ),
        }
    }
}
trait RenderWedge {
    fn render_geometry(
        &self,
        scene: &mut vello::Scene,
        parent_transform: Affine,
        inherited_opacity: f32,
        geometry: Geometry,
    );
}
// The transform and opacity a pointed shape draws with, or `None` if it
// would draw nothing.
fn wedge_frame(
    size: f64,
    opacity: f64,
    inherited_opacity: f32,
    transform: Affine,
) -> Option<(Affine, f32)> {
    let opacity = opacity.clamp(0.0, 1.0) as f32 * inherited_opacity;
    (size > 0.0 && opacity > 0.0 && transform.determinant() != 0.0)
        .then_some((transform, opacity))
}
// Stroke `outline` unless the stroke is invisible.
#[expect(
    clippy::too_many_arguments,
    reason = "one stroke style, passed as its parts"
)]
fn stroke_outline(
    scene: &mut vello::Scene,
    transform: Affine,
    opacity: f32,
    color: Color,
    width: f64,
    cap: Cap,
    join: Join,
    outline: &[PathEl],
) {
    if width > 0.0 && color.components[3] > 0.0 {
        scene.stroke(
            &Stroke::new(width).with_caps(cap).with_join(join),
            transform,
            color.with_alpha(color.components[3] * opacity),
            None,
            &outline,
        );
    }
}
impl RenderWedge for Triangle {
    fn render_geometry(
        &self,
        scene: &mut vello::Scene,
        parent_transform: Affine,
        inherited_opacity: f32,
        geometry: Geometry,
    ) {
        let Some((transform, opacity)) = wedge_frame(
            self.size,
            self.opacity,
            inherited_opacity,
            parent_transform * self.transform(),
        ) else {
            return;
        };
        let geometry = geometry.canvas();
        let outline = geometry.elements();
        if self.fill.components[3] > 0.0 {
            scene.fill(
                Fill::NonZero,
                transform,
                self.fill
                    .with_alpha(self.fill.components[3] * opacity),
                None,
                &outline,
            );
        }
        stroke_outline(
            scene,
            transform,
            opacity,
            self.stroke,
            self.stroke_width,
            self.line_cap,
            self.line_join,
            outline,
        );
    }
}
impl RenderWedge for Chevron {
    fn render_geometry(
        &self,
        scene: &mut vello::Scene,
        parent_transform: Affine,
        inherited_opacity: f32,
        geometry: Geometry,
    ) {
        let Some((transform, opacity)) = wedge_frame(
            self.size,
            self.opacity,
            inherited_opacity,
            parent_transform * self.transform(),
        ) else {
            return;
        };
        let geometry = geometry.canvas();
        stroke_outline(
            scene,
            transform,
            opacity,
            self.stroke,
            self.stroke_width,
            self.line_cap,
            self.line_join,
            geometry.elements(),
        );
    }
}
/// Draws sampled 2D scenes into a `vello::Scene`. Keep one per window and
/// reuse it: it caches path geometry, text layout and rounded triangles
/// between frames and rebuilds an entry only when its object changes.
///
/// ```
/// use motiongfx::prelude::s;
/// use motiongfx_2d::prelude::*;
/// use vello_motiongfx::Renderer;
///
/// let mut scene = Scene::new();
/// let dot = scene.add(Circle::new(12.0));
/// let mut animation = scene.compile([dot.position_x().to(100.0)]);
/// let mut renderer = Renderer::default();
/// let mut frame = vello::Scene::new();
/// let size = Size::new(800.0, 600.0);
/// animation.sample(s(1));
/// renderer.render(&animation, &mut frame, size, size, 1.0);
/// ```
#[derive(Default)]
pub struct Renderer {
    text_fonts: Option<motiongfx_text::FontSystem>,
    pub(crate) path_geometry: Vec<path_renderer::PathGeometry>,
    pub(crate) text_layouts: Vec<text_renderer::TextLayout>,
    pub(crate) triangle_geometry: Vec<CachedTriangle>,
}
impl Renderer {
    /// The fonts text is shaped with (Fira Sans is built in). Register your
    /// own fonts here before rendering text that uses them.
    pub fn fonts_mut(&mut self) -> &mut motiongfx_text::FontSystem {
        self.text_fonts.get_or_insert_with(Default::default)
    }
    /// Draw `animation` as last sampled into `scene`. The `canvas` (the
    /// scene's coordinate space, centred on the origin) is scaled to fit
    /// `viewport` (logical pixels), then multiplied by `dpi`.
    pub fn render<M: Model2d>(
        &mut self,
        animation: &motiongfx_authoring::CompiledScene<M>,
        scene: &mut vello::Scene,
        viewport: Size,
        canvas: Size,
        dpi: f64,
    ) {
        self.render_canvas(
            animation.model().canvas(),
            scene,
            viewport,
            canvas,
            dpi,
        );
    }
    /// Like [`render`](Self::render), from the 2D objects directly.
    pub fn render_canvas(
        &mut self,
        graphics: &Canvas,
        scene: &mut vello::Scene,
        viewport: Size,
        canvas: Size,
        dpi: f64,
    ) {
        let fit = (viewport.width / canvas.width)
            .min(viewport.height / canvas.height);
        let screen = Affine::scale(dpi)
            * Affine::translate((
                viewport.width / 2.0,
                viewport.height / 2.0,
            ))
            * Affine::scale(fit)
            * graphics.camera_transform();
        self.path_geometry.truncate(graphics.paths().len());
        for path in &graphics.paths()[self.path_geometry.len()..] {
            self.path_geometry
                .push(path_renderer::PathGeometry::new(path));
        }
        self.text_layouts.truncate(graphics.texts().len());
        for _ in &graphics.texts()[self.text_layouts.len()..] {
            self.text_layouts
                .push(text_renderer::TextLayout::default());
        }
        self.triangle_geometry.truncate(graphics.triangles().len());
        for triangle in
            &graphics.triangles()[self.triangle_geometry.len()..]
        {
            self.triangle_geometry
                .push(CachedTriangle::new(triangle));
        }
        for (triangle, cache) in graphics
            .triangles()
            .iter()
            .zip(&mut self.triangle_geometry)
        {
            cache.update(triangle);
        }
        for object in graphics.draw_order() {
            match *object {
                DrawObject::Line(id) => graphics.lines()[id.index()]
                    .render(
                        scene,
                        screen,
                        graphics,
                        &self.triangle_geometry,
                    ),
                DrawObject::Triangle(id) => {
                    graphics.triangles()[id.index()].render_geometry(
                        scene,
                        screen,
                        1.0,
                        self.triangle_geometry[id.index()].geometry,
                    )
                }
                DrawObject::Chevron(id) => {
                    graphics.chevrons()[id.index()].render_geometry(
                        scene,
                        screen,
                        1.0,
                        graphics.chevrons()[id.index()].geometry(),
                    )
                }
                DrawObject::Circle(id) => {
                    let circle = &graphics.circles()[id.index()];
                    circle.render(
                        scene,
                        screen
                            * graphics
                                .circle_parent_transform(circle),
                        1.0,
                    );
                }
                DrawObject::Rectangle(id) => graphics.rectangles()
                    [id.index()]
                .render(scene, screen),
                DrawObject::Path(id) => self.path_geometry
                    [id.index()]
                .render(&graphics.paths()[id.index()], scene, screen),
                DrawObject::Text(id) => {
                    let text = &graphics.texts()[id.index()];
                    self.text_layouts[id.index()].render(
                        text,
                        scene,
                        screen * graphics.text_parent_transform(text),
                        &mut self.text_fonts,
                        graphics.text_selections(),
                    );
                }
            }
        }
    }
}
