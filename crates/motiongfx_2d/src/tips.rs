//! Endpoint placement is separate from the ordinary shape implementation.

use crate::*;
use motiongfx::prelude::ThreadSafe;
use std::ops::Deref;

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
    impl<M: crate::Model2d> Sealed for crate::Object<crate::Circle, M> {}
    impl<M: crate::Model2d> Sealed for crate::Object<crate::Triangle, M> {}
    impl<M: crate::Model2d> Sealed for crate::Object<crate::Chevron, M> {}
}
/// Supported inline tip descriptions. Registration retains the shape's concrete
/// handle type. Tips are drawn by their line, not as independent draw entries.
pub trait LineTip<M: Model2d = Graphics>: sealed::Sealed {
    type Handle: Copy + ThreadSafe;
    #[doc(hidden)]
    fn attach(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> (Self::Handle, Option<EndpointTip>);
}
impl<M: Model2d> LineTip<M> for NoTip {
    type Handle = NoTip;
    fn attach(
        self,
        _: &mut motiongfx_authoring::Scene<M>,
    ) -> (NoTip, Option<EndpointTip>) {
        (self, None)
    }
}
impl<M: Model2d> LineTip<M> for Object<Circle, M> {
    type Handle = CircleHandle;
    fn attach(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
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
        impl<M: Model2d> LineTip<M> for Object<$shape, M> {
            type Handle = $handle;
            fn attach(
                self,
                scene: &mut motiongfx_authoring::Scene<M>,
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
    pub(crate) fn extent(
        self,
        graphics: &Graphics,
        triangle_geometry: impl FnOnce(TriangleId) -> Geometry,
    ) -> f64 {
        match self {
            Self::Circle(id) => {
                let c = &graphics.circles[id.0];
                (c.radius.max(0.0) * c.scale.abs() - c.position.x)
                    .max(0.0)
            }
            Self::Triangle(id) => {
                let triangle = &graphics.triangles[id.0];
                triangle_geometry(id).extent(
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
}

/// Construction-only wrapper retaining each tip's concrete shape type.
#[derive(Clone)]
pub struct LineWithTips<S = NoTip, E = NoTip, M: Model2d = Graphics> {
    line: Object<Line, M>,
    start_tip: S,
    end_tip: E,
}
impl<M: Model2d> Object<Line, M> {
    /// Attach an ordinary shape at the start. Its style stays independent;
    /// position and rotation become local offsets in the outward endpoint frame.
    ///
    /// ```
    /// use motiongfx::prelude::s;
    /// use motiongfx_2d::{Animation2dExt,Circle, Line, Scene, Triangle};
    /// let mut scene = Scene::new();
    /// let line = scene.add(Line::new((0.0, 0.0), (200.0, 0.0))
    ///     .start_tip(Circle::new(6.0))
    ///     .end_tip(Triangle::new().size(24.0).angle(0.6)));
    /// let mut animation = scene.animate(|sequence| {
    ///     sequence.play(line.start_tip.radius().to(10.0));
    ///     sequence.play(line.end_tip.angle().to(0.9));
    /// });
    /// animation.sample(s(2));
    /// assert_eq!(animation.circle(line.start_tip).radius, 10.0);
    /// ```
    /// Tip-specific properties are checked at compile time:
    /// ```compile_fail
    /// use motiongfx_2d::{Animation2dExt,Circle, Line, Scene};
    /// let mut scene = Scene::new();
    /// let line = scene.add(Line::new((0.0, 0.0), (200.0, 0.0))
    ///     .start_tip(Circle::new(6.0)));
    /// line.start_tip.angle().to(0.6); // A circle has radius, not an opening angle.
    /// ```
    pub fn start_tip<S: LineTip<M>>(
        self,
        tip: S,
    ) -> LineWithTips<S, NoTip, M> {
        LineWithTips {
            line: self,
            start_tip: tip,
            end_tip: NoTip,
        }
    }
    pub fn end_tip<E: LineTip<M>>(
        self,
        tip: E,
    ) -> LineWithTips<NoTip, E, M> {
        LineWithTips {
            line: self,
            start_tip: NoTip,
            end_tip: tip,
        }
    }
}
impl<S, E, M: Model2d> LineWithTips<S, E, M> {
    pub fn start_tip<T: LineTip<M>>(
        self,
        tip: T,
    ) -> LineWithTips<T, E, M> {
        LineWithTips {
            line: self.line,
            start_tip: tip,
            end_tip: self.end_tip,
        }
    }
    pub fn end_tip<T: LineTip<M>>(
        self,
        tip: T,
    ) -> LineWithTips<S, T, M> {
        LineWithTips {
            line: self.line,
            start_tip: self.start_tip,
            end_tip: tip,
        }
    }
    pub fn stroke(
        mut self,
        color: impl Into<Input<Color, M>>,
        width: impl Into<Input<f64, M>>,
    ) -> Self {
        self.line = self.line.stroke(color, width);
        self
    }
}
// Forward only construction setters; runtime properties remain ordinary Line
// properties, reached through the returned handle's Deref implementation.
macro_rules! setters {
    ($($field:ident: $ty:ty),* $(,)?) => {
        impl<S,E,M:Model2d> LineWithTips<S,E,M> {
            $(pub fn $field(mut self, value: impl Into<Input<$ty,M>>) -> Self {
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
impl<M: Model2d, S: LineTip<M>, E: LineTip<M>> SceneObject<M>
    for LineWithTips<S, E, M>
{
    type Handle = LineTipHandle<S::Handle, E::Handle>;
    fn insert_into(
        self,
        scene: &mut motiongfx_authoring::Scene<M>,
    ) -> Self::Handle {
        let line = self.line.insert_into(scene);
        let (start_tip, start) = self.start_tip.attach(scene);
        let (end_tip, end) = self.end_tip.attach(scene);
        scene.model_mut().graphics_mut().lines[line.id.0].start_tip =
            start;
        scene.model_mut().graphics_mut().lines[line.id.0].end_tip =
            end;
        LineTipHandle {
            line,
            start_tip,
            end_tip,
        }
    }
}
