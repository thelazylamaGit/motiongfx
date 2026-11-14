//! Compound position handles retain whole-point operations and expose axes.

use crate::Point;
use motiongfx::prelude::Path;
use motiongfx_authoring::Property;
use std::ops::Deref;

pub struct Position<
    I,
    S,
    P = Property<I, S, Point>,
    X = Property<I, S, f64>,
    Y = Property<I, S, f64>,
> {
    whole: P,
    pub x: X,
    pub y: Y,
    marker: std::marker::PhantomData<fn() -> (I, S)>,
}

impl<I, S, P: Copy, X: Copy, Y: Copy> Copy
    for Position<I, S, P, X, Y>
{
}
impl<I, S, P: Clone, X: Clone, Y: Clone> Clone
    for Position<I, S, P, X, Y>
{
    fn clone(&self) -> Self {
        Self::from_targets(
            self.whole.clone(),
            self.x.clone(),
            self.y.clone(),
        )
    }
}

impl<I: Copy, S> Position<I, S> {
    pub fn new(
        id: I,
        whole: Path<S, Point>,
        x: Path<S, f64>,
        y: Path<S, f64>,
    ) -> Self {
        Self {
            whole: Property::new(id, whole),
            x: Property::new(id, x),
            y: Property::new(id, y),
            marker: std::marker::PhantomData,
        }
    }
}

impl<I, S, P, X, Y> Position<I, S, P, X, Y> {
    pub fn from_targets(whole: P, x: X, y: Y) -> Self {
        Self {
            whole,
            x,
            y,
            marker: std::marker::PhantomData,
        }
    }
}
impl<I, S, P, X, Y> Deref for Position<I, S, P, X, Y> {
    type Target = P;
    fn deref(&self) -> &P {
        &self.whole
    }
}
impl<I, S, P: Into<Property<I, S, Point>>, X, Y>
    From<Position<I, S, P, X, Y>> for Property<I, S, Point>
{
    fn from(position: Position<I, S, P, X, Y>) -> Self {
        position.whole.into()
    }
}
impl<W, I, S, P, X, Y> motiongfx_authoring::Source<W>
    for Position<I, S, P, X, Y>
where
    P: motiongfx_authoring::Source<W, Value = Point>,
{
    type Value = Point;
    type Prepared = P::Prepared;
    type Owned = P::Owned;
    fn owned(&self) -> Self::Owned {
        self.whole.owned()
    }
    fn register(
        &self,
        world: &mut W,
        plan: &mut motiongfx_authoring::BindingPlan<W>,
    ) -> Self::Prepared {
        self.whole.register(world, plan)
    }
}
impl<W, I: 'static, S: 'static, P, X, Y>
    motiongfx_authoring::BindingSource<W> for Position<I, S, P, X, Y>
where
    P: motiongfx_authoring::BindingSource<W, Value = Point>,
    X: Clone + Send + Sync + 'static,
    Y: Clone + Send + Sync + 'static,
{
    type Value = Point;
    fn read(
        &self,
        world: &W,
        values: &motiongfx_authoring::bindings::DerivedValues,
    ) -> Option<Point> {
        self.whole.read(world, values)
    }
    fn dependencies(
        &self,
    ) -> Vec<motiongfx_authoring::bindings::BindingDependency> {
        self.whole.dependencies()
    }
    fn validate(
        &self,
        world: &W,
    ) -> Result<(), motiongfx_authoring::BindingError> {
        self.whole.validate(world)
    }
}

// Declare field identity beside the owning object. Markers contain no data;
// only a Target's subject ID occupies storage. General Property expansion is
// shared with the ordinary animation/binding implementation.
macro_rules! fields {
    ($module:ident, $state:ty, $id:ty; $( $name:ident: $value:ty => $method:ident($($path:tt)+); )*) => {
        pub mod $module {
            use super::*;
            $(pub mod $name {
                use super::*;
                pub struct Marker;
                pub type Target = motiongfx_authoring::Target<$id,$state,$value,Marker>;
                impl motiongfx_authoring::Field<$state,$value> for Marker {
                    fn property<I:Copy>(id:I)->motiongfx_authoring::Property<I,$state,$value> {
                        motiongfx_authoring::Property::$method(id,motiongfx::path!($($path)+))
                    }
                }
            })*
        }
    }
}
pub(crate) use fields;

// Getter methods materialize only one typed target; each object handle owns one ID.
macro_rules! handle_methods {
    ($handle:ident, $state:ty, $id:ty; $($name:ident: $value:ty;)*) => {
        impl $handle {
            $(pub fn $name(self) -> fields::$name::Target {
                motiongfx_authoring::Target::new(self.id)
            })*
        }
    }
}
pub(crate) use handle_methods;

macro_rules! fill_methods {
    ($handle:ident,$state:ty,$id:ty) => {
        impl $handle {
            /// Animate fill using the scene defaults unless timed.
            #[track_caller]
            pub fn fill_to<M: crate::Model2d>(
                self,
                color: impl Into<crate::Color>,
            ) -> motiongfx_authoring::Animation<M> {
                self.fill().to(color)
            }
            /// Replace fill at the scheduled time without consuming duration.
            #[track_caller]
            pub fn set_fill<M: crate::Model2d>(
                self,
                color: impl Into<crate::Color>,
            ) -> motiongfx_authoring::Animation<M> {
                self.fill().set(color)
            }
        }
    };
}
pub(crate) use fill_methods;

macro_rules! transform_methods {
    ($handle:ident,$state:ty,$id:ty) => {
        impl $handle {
            /// Offset position from its action-start value.
            #[track_caller]
            pub fn shift<M: crate::Model2d>(
                self,
                x: f64,
                y: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.position().offset((x, y))
            }
            #[track_caller]
            pub fn shift_x<M: crate::Model2d>(
                self,
                x: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.position_x().offset(x)
            }
            #[track_caller]
            pub fn shift_y<M: crate::Model2d>(
                self,
                y: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.position_y().offset(y)
            }
            /// Multiply scale from its action-start value.
            #[track_caller]
            pub fn scale_by<M: crate::Model2d>(
                self,
                factor: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.scale().multiply(factor)
            }
            /// Offset rotation in counterclockwise radians.
            #[track_caller]
            pub fn rotate<M: crate::Model2d>(
                self,
                angle: f64,
            ) -> motiongfx_authoring::Animation<M> {
                self.rotation().offset(angle)
            }
        }
    };
}
pub(crate) use transform_methods;
