//! Endpoint placement is separate from the ordinary shape implementation.

use crate::*;
use std::ops::Deref;
use vello::kurbo::Affine;

/// Marker for an endpoint without a tip. It has no animation properties.
#[derive(Clone, Copy, Default)]
pub struct NoTip;

#[doc(hidden)]
#[derive(Clone, Copy)]
pub enum EndpointTip {
    Circle(CircleId),
    Triangle(TriangleId),
    Chevron(ChevronId),
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::NoTip {}
    impl Sealed for crate::Object<crate::Circle> {}
    impl Sealed for crate::Object<crate::Triangle> {}
    impl Sealed for crate::Object<crate::Chevron> {}
}
/// Supported inline tip descriptions. Registration retains the shape's concrete
/// handle type. Tips are drawn by their line, not as independent draw entries.
pub trait LineTip: sealed::Sealed {
    type Handle: Copy + ThreadSafe;
    #[doc(hidden)]
    fn attach(
        self,
        scene: &mut Scene,
    ) -> (Self::Handle, Option<EndpointTip>);
}
impl LineTip for NoTip {
    type Handle = NoTip;
    fn attach(self, _: &mut Scene) -> (NoTip, Option<EndpointTip>) {
        (self, None)
    }
}
impl LineTip for Object<Circle> {
    type Handle = CircleHandle;
    fn attach(
        self,
        scene: &mut Scene,
    ) -> (CircleHandle, Option<EndpointTip>) {
        assert!(
            self.parent.is_none(),
            "a line tip uses its endpoint frame; remove relative_to"
        );
        let handle = self.insert_with(scene, |shape, scene| {
            shape.insert(scene, false)
        });
        (handle, Some(EndpointTip::Circle(handle.id)))
    }
}
macro_rules! attach {
    ($shape:ident, $handle:ident) => {
        impl LineTip for Object<$shape> {
            type Handle = $handle;
            fn attach(
                self,
                scene: &mut Scene,
            ) -> ($handle, Option<EndpointTip>) {
                let handle = self
                    .insert_with(scene, |shape, scene| {
                        shape.insert(scene, false)
                    });
                (handle, Some(EndpointTip::$shape(handle.id)))
            }
        }
    };
}
attach!(Triangle, TriangleHandle);
attach!(Chevron, ChevronHandle);

impl EndpointTip {
    pub(crate) fn extent(self, graphics: &Graphics) -> f64 {
        match self {
            Self::Circle(id) => {
                let c = &graphics.circles[id.0];
                (c.radius.max(0.0) * c.scale.abs() - c.position.x)
                    .max(0.0)
            }
            Self::Triangle(id) => {
                let triangle = &graphics.triangles[id.0];
                graphics.triangle_geometry[id.0].geometry.extent(
                    triangle.position,
                    triangle.rotation,
                    triangle.scale,
                )
            }
            Self::Chevron(id) => graphics.chevrons[id.0].extent(),
        }
    }
    pub(crate) fn trim(self, extent: f64) -> f64 {
        // An open chevron meets the shaft at its apex. Filled shapes cover the
        // shaft up to their back edge (circle: its inward-facing circumference).
        match self {
            Self::Chevron(_) => 0.0,
            _ => extent,
        }
    }
    pub(crate) fn render(
        self,
        graphics: &Graphics,
        scene: &mut vello::Scene,
        transform: Affine,
        inherited_opacity: f32,
    ) {
        match self {
            Self::Circle(id) => graphics.circles[id.0].render(
                scene,
                transform,
                inherited_opacity,
            ),
            Self::Triangle(id) => graphics.triangles[id.0].render(
                scene,
                transform,
                inherited_opacity,
                graphics.triangle_geometry[id.0].geometry,
            ),
            Self::Chevron(id) => graphics.chevrons[id.0].render(
                scene,
                transform,
                inherited_opacity,
            ),
        }
    }
}

/// Construction-only wrapper retaining each tip's concrete shape type.
#[derive(Clone)]
pub struct LineWithTips<S = NoTip, E = NoTip> {
    line: Object<Line>,
    start_tip: S,
    end_tip: E,
}
impl Object<Line> {
    /// Attach an ordinary shape at the start. Its style stays independent;
    /// position and rotation become local offsets in the outward endpoint frame.
    ///
    /// ```
    /// use motiongfx::prelude::s;
    /// use vello_motiongfx::{Circle, Line, Scene, Triangle};
    /// let mut scene = Scene::new();
    /// let line = scene.add(Line::new((0.0, 0.0), (200.0, 0.0))
    ///     .start_tip(Circle::new(6.0))
    ///     .end_tip(Triangle::new().size(24.0).angle(0.6)));
    /// let mut animation = scene.animate(|sequence| {
    ///     sequence.play(line.start_tip.radius.to(10.0));
    ///     sequence.play(line.end_tip.angle.to(0.9));
    /// });
    /// animation.sample(s(2));
    /// assert_eq!(animation.circle(line.start_tip).radius, 10.0);
    /// ```
    /// Tip-specific properties are checked at compile time:
    /// ```compile_fail
    /// use vello_motiongfx::{Circle, Line, Scene};
    /// let mut scene = Scene::new();
    /// let line = scene.add(Line::new((0.0, 0.0), (200.0, 0.0))
    ///     .start_tip(Circle::new(6.0)));
    /// line.start_tip.angle.to(0.6); // A circle has radius, not an opening angle.
    /// ```
    pub fn start_tip<S: LineTip>(self, tip: S) -> LineWithTips<S> {
        LineWithTips {
            line: self,
            start_tip: tip,
            end_tip: NoTip,
        }
    }
    pub fn end_tip<E: LineTip>(
        self,
        tip: E,
    ) -> LineWithTips<NoTip, E> {
        LineWithTips {
            line: self,
            start_tip: NoTip,
            end_tip: tip,
        }
    }
}
impl<S, E> LineWithTips<S, E> {
    pub fn start_tip<T: LineTip>(self, tip: T) -> LineWithTips<T, E> {
        LineWithTips {
            line: self.line,
            start_tip: tip,
            end_tip: self.end_tip,
        }
    }
    pub fn end_tip<T: LineTip>(self, tip: T) -> LineWithTips<S, T> {
        LineWithTips {
            line: self.line,
            start_tip: self.start_tip,
            end_tip: tip,
        }
    }
    pub fn stroke(
        mut self,
        color: impl Into<Input<Color>>,
        width: impl Into<Input<f64>>,
    ) -> Self {
        self.line = self.line.stroke(color, width);
        self
    }
}
// Forward only construction setters; runtime properties remain ordinary Line
// properties, reached through the returned handle's Deref implementation.
macro_rules! setters {
    ($($field:ident: $ty:ty),* $(,)?) => {
        impl<S, E> LineWithTips<S, E> {
            $(pub fn $field(mut self, value: impl Into<Input<$ty>>) -> Self {
                self.line = self.line.$field(value); self
            })*
        }
    };
}
setters!(start: Point, end: Point, opacity: f64, stroke_width: f64,
    line_cap: Cap, line_join: Join,
    reveal: f64, line_dash: std::sync::Arc<[f64]>, dash_offset: f64,
    start_offset: f64, end_offset: f64,);

/// Ordinary line properties plus statically typed endpoint shape handles.
#[derive(Clone, Copy)]
pub struct LineTipHandle<S, E> {
    pub(crate) line: LineHandle,
    pub start_tip: S,
    pub end_tip: E,
}
impl<S, E> Deref for LineTipHandle<S, E> {
    type Target = LineHandle;
    fn deref(&self) -> &LineHandle {
        &self.line
    }
}
impl<S, E> From<LineTipHandle<S, E>> for LineHandle {
    fn from(handle: LineTipHandle<S, E>) -> Self {
        handle.line
    }
}
impl<S: LineTip, E: LineTip> SceneObject for LineWithTips<S, E> {
    type Handle = LineTipHandle<S::Handle, E::Handle>;
    fn insert_into(self, scene: &mut Scene) -> Self::Handle {
        let line = self.line.insert_into(scene);
        let (start_tip, start) = self.start_tip.attach(scene);
        let (end_tip, end) = self.end_tip.attach(scene);
        scene.graphics.lines[line.id.0].start_tip = start;
        scene.graphics.lines[line.id.0].end_tip = end;
        LineTipHandle {
            line,
            start_tip,
            end_tip,
        }
    }
}
