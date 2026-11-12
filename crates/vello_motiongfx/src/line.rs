//! A styled line with independently animatable endpoints; no path allocation.

use crate::tips::EndpointTip;
use crate::{
    Color, DrawObject, Graphics, Point, Position, Scene, SceneObject,
};
use motiongfx::prelude::{SubjectSource, path};
use motiongfx_authoring::Property;
use std::sync::Arc;
use vello::kurbo::{Affine, Line as LineShape, Stroke};

#[derive(Clone)]
pub struct Line {
    pub start: Point,
    pub end: Point,
    pub stroke: Color,
    pub stroke_width: f64,
    pub line_cap: crate::Cap,
    pub line_join: crate::Join,
    pub opacity: f64,
    /// Visible fraction from start towards end, clamped to 0..1 in rendering.
    pub reveal: f64,
    pub line_dash: Arc<[f64]>,
    pub dash_offset: f64,
    pub start_offset: f64,
    pub end_offset: f64,
    pub(crate) start_tip: Option<EndpointTip>,
    pub(crate) end_tip: Option<EndpointTip>,
}
impl Line {
    pub fn new(
        start: impl Into<crate::Input<Point>>,
        end: impl Into<crate::Input<Point>>,
    ) -> crate::Object<Self> {
        crate::Object::new(Self {
            start: Point::ZERO,
            end: Point::ZERO,
            stroke: Color::WHITE,
            stroke_width: 1.0,
            line_cap: crate::Cap::Round,
            line_join: crate::Join::Round,
            opacity: 1.0,
            reveal: 1.0,
            line_dash: Arc::from([]),
            dash_offset: 0.0,
            start_offset: 0.0,
            end_offset: 0.0,
            start_tip: None,
            end_tip: None,
        })
        .start(start)
        .end(end)
    }
    pub(crate) fn render(
        &self,
        scene: &mut vello::Scene,
        screen: Affine,
        graphics: &Graphics,
    ) {
        let opacity = self.opacity.clamp(0.0, 1.0) as f32;
        if opacity <= 0.0 || self.reveal <= 0.0 {
            return;
        }
        if self.start_tip.is_none()
            && self.end_tip.is_none()
            && self.start_offset <= 0.0
            && self.end_offset <= 0.0
        {
            // Ordinary lines need no length, orientation or tip calculation.
            let end = self.start
                + (self.end - self.start)
                    * self.reveal.clamp(0.0, 1.0);
            self.draw_shaft(scene, screen, self.start, end, opacity);
            return;
        }
        let delta = self.end - self.start;
        let length = delta.hypot();
        if !length.is_finite() || length <= 0.0 {
            return;
        }
        let direction = delta / length;
        let distance = (length
            - self.start_offset.max(0.0)
            - self.end_offset.max(0.0))
        .max(0.0)
            * self.reveal.clamp(0.0, 1.0);
        if distance <= 0.0 {
            return;
        }
        let tip_start = self.start
            + direction * self.start_offset.max(0.0).min(length);
        let tip_end = tip_start + direction * distance;
        let start_extent =
            self.start_tip.map_or(0.0, |t| t.extent(graphics));
        let end_extent =
            self.end_tip.map_or(0.0, |t| t.extent(graphics));
        let extent = start_extent + end_extent;
        // Fit both tips together as the line is revealed; never reverse the shaft.
        let fit = if extent > distance {
            distance / extent
        } else {
            1.0
        };
        let start = tip_start
            + direction
                * self
                    .start_tip
                    .map_or(0.0, |t| t.trim(start_extent))
                * fit;
        let end = tip_end
            - direction
                * self.end_tip.map_or(0.0, |t| t.trim(end_extent))
                * fit;
        if start.distance_squared(end) > 1e-18 {
            self.draw_shaft(scene, screen, start, end, opacity);
        }
        let angle = direction.y.atan2(direction.x);
        for (tip, position, angle) in [
            (self.start_tip, tip_start, angle + std::f64::consts::PI),
            (self.end_tip, tip_end, angle),
        ] {
            if let Some(tip) = tip {
                let frame = screen
                    * Affine::translate((position.x, -position.y))
                    * Affine::rotate(-angle)
                    * Affine::scale(fit);
                tip.render(graphics, scene, frame, opacity);
            }
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
crate::object::properties!(Line;
    start: Point, end: Point, opacity: f64, stroke_width: f64, reveal: f64,
    line_dash: Arc<[f64]>, dash_offset: f64, start_offset: f64, end_offset: f64,

);
crate::object::stroke!(Line);
crate::object::stroke_options!(Line);

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct LineId(pub(crate) usize);
#[derive(Clone, Copy)]
pub struct LineHandle {
    pub(crate) id: LineId,
    pub start: Position<LineId, Line>,
    pub end: Position<LineId, Line>,
    pub stroke: Property<LineId, Line, Color>,
    pub stroke_width: Property<LineId, Line, f64>,
    pub line_cap: Property<LineId, Line, crate::Cap>,
    pub line_join: Property<LineId, Line, crate::Join>,
    pub opacity: Property<LineId, Line, f64>,
    pub reveal: Property<LineId, Line, f64>,
    pub line_dash: Property<LineId, Line, Arc<[f64]>>,
    pub dash_offset: Property<LineId, Line, f64>,
    pub start_offset: Property<LineId, Line, f64>,
    pub end_offset: Property<LineId, Line, f64>,
}
impl SceneObject for Line {
    type Handle = LineHandle;
    fn insert_into(self, scene: &mut Scene) -> LineHandle {
        let id = LineId(scene.graphics.lines.len());
        scene.graphics.lines.push(self);
        scene.graphics.order.push(DrawObject::Line(id));
        LineHandle {
            id,
            start: Position::new(
                id,
                path!(Line.start),
                path!(Line.start.x),
                path!(Line.start.y),
            ),
            end: Position::new(
                id,
                path!(Line.end),
                path!(Line.end.x),
                path!(Line.end.y),
            ),
            stroke: Property::new(id, path!(Line.stroke)),
            line_cap: Property::discrete(id, path!(Line.line_cap)),
            line_join: Property::discrete(id, path!(Line.line_join)),
            stroke_width: Property::new(id, path!(Line.stroke_width)),
            opacity: Property::new(id, path!(Line.opacity)),
            reveal: Property::new(id, path!(Line.reveal)),
            line_dash: Property::discrete(id, path!(Line.line_dash)),
            dash_offset: Property::new(id, path!(Line.dash_offset)),
            start_offset: Property::new(id, path!(Line.start_offset)),
            end_offset: Property::new(id, path!(Line.end_offset)),
        }
    }
}
impl SubjectSource<LineId, Line> for Graphics {
    fn get_source(&self, id: LineId) -> Option<&Line> {
        self.lines.get(id.0)
    }
    fn apply_source<R>(
        &mut self,
        id: LineId,
        f: impl FnOnce(&mut Line) -> R,
    ) -> Option<R> {
        self.lines.get_mut(id.0).map(f)
    }
}

impl crate::Animation {
    pub fn line(&self, handle: impl Into<LineHandle>) -> &Line {
        &self.graphics.lines[handle.into().id.0]
    }
}
