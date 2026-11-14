//! Vello command emission. Sampled object types remain renderer-independent.
use crate::*;
use vello::kurbo::{
    Affine, Circle as CircleShape, Line as LineShape, Stroke,
};
use vello::peniko::Fill;
trait RenderRectangle {
    fn render(&self, scene: &mut vello::Scene, screen: Affine);
}
impl RenderRectangle for Rectangle {
    fn render(&self, scene: &mut vello::Scene, screen: Affine) {
        let rectangle = self;
        let xf = screen * self.transform();
        let half_w = rectangle.size.width * rectangle.scale / 2.0;
        let half_h = rectangle.size.height * rectangle.scale / 2.0;
        let shape = Rect::new(-half_w, -half_h, half_w, half_h)
            .to_rounded_rect(
                rectangle.radius * rectangle.scale.abs(),
            );
        scene.fill(Fill::NonZero, xf, rectangle.fill, None, &shape);
        if rectangle.stroke_width > 0.0 {
            scene.stroke(
                &Stroke::new(rectangle.stroke_width)
                    .with_caps(rectangle.line_cap)
                    .with_join(rectangle.line_join),
                xf,
                rectangle.stroke,
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
        graphics: &Graphics,
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
        graphics: &Graphics,
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
        graphics: &Graphics,
        triangle_geometry: &[CachedTriangle],
        scene: &mut vello::Scene,
        transform: Affine,
        inherited_opacity: f32,
    );
}
impl RenderTip for EndpointTip {
    fn render(
        self,
        graphics: &Graphics,
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
macro_rules! wedge_renderer {
    ($shape:ty,$closed:expr) => {
        impl RenderWedge for $shape {
            fn render_geometry(
                &self,
                scene: &mut vello::Scene,
                parent_transform: Affine,
                inherited_opacity: f32,
                geometry: Geometry,
            ) {
                let opacity = self.opacity.clamp(0.0, 1.0) as f32
                    * inherited_opacity;
                let transform = parent_transform * self.transform();
                if self.size <= 0.0
                    || opacity <= 0.0
                    || transform.determinant() == 0.0
                {
                    return;
                }
                let geometry = geometry.canvas();
                let outline = geometry.elements();
                if $closed && self.fill.components[3] > 0.0 {
                    scene.fill(
                        Fill::NonZero,
                        transform,
                        self.fill.with_alpha(
                            self.fill.components[3] * opacity,
                        ),
                        None,
                        &outline,
                    );
                }
                if self.stroke_width > 0.0
                    && self.stroke.components[3] > 0.0
                {
                    scene.stroke(
                        &Stroke::new(self.stroke_width)
                            .with_caps(self.line_cap)
                            .with_join(self.line_join),
                        transform,
                        self.stroke.with_alpha(
                            self.stroke.components[3] * opacity,
                        ),
                        None,
                        &outline,
                    );
                }
            }
        }
    };
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
wedge_renderer!(Triangle, true);
wedge_renderer!(Chevron, false);
/// Records Vello commands and owns renderer-specific geometry/text caches.
#[derive(Default)]
pub struct Renderer {
    text_fonts: Option<motiongfx_text::FontSystem>,
    pub(crate) path_geometry: Vec<path_renderer::PathGeometry>,
    pub(crate) text_layouts: Vec<text_renderer::TextLayout>,
    pub(crate) triangle_geometry: Vec<CachedTriangle>,
}
impl Renderer {
    /// Lazily create the shared font/layout resources, including embedded Fira
    /// Sans. Register application fonts here before rendering font-backed text.
    pub fn fonts_mut(&mut self) -> &mut motiongfx_text::FontSystem {
        self.text_fonts.get_or_insert_with(Default::default)
    }
    pub fn render<M: Model2d>(
        &mut self,
        animation: &motiongfx_authoring::CompiledScene<M>,
        scene: &mut vello::Scene,
        viewport: Size,
        canvas: Size,
        dpi: f64,
    ) {
        self.render_graphics(
            animation.model().graphics(),
            scene,
            viewport,
            canvas,
            dpi,
        );
    }
    pub fn render_graphics(
        &mut self,
        graphics: &Graphics,
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
