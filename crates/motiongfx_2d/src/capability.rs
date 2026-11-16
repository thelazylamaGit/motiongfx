//! Traits for what several kinds of objects share, such as a position or
//! a fill, so one function can animate any object that has it.
use crate::{Canvas, Color, Model2d, Point};
use motiongfx_authoring::{Animation, Field, ObjectHandle, Target};

/// The field of `H` identified by marker `F`.
type FieldOf<H, M, T, F> = Target<
    <H as ObjectHandle<M>>::Id,
    <H as ObjectHandle<M>>::State,
    T,
    F,
>;

/// Objects with a position, scale and rotation: shapes, text and the
/// camera.
///
/// # Examples
///
/// One function for any object that moves and fades:
///
/// ```
/// use motiongfx::prelude::s;
/// use motiongfx_2d::prelude::*;
///
/// fn rise_in(object: impl Transformable + Fadable) -> Animation {
///     all([object.shift_y(20.0), object.opacity().to(1.0)])
/// }
///
/// let mut scene = Scene::new();
/// let dot = scene.add(Circle::new(10.0).opacity(0.0));
/// let label = scene.add(Text::new("Hi").opacity(0.0));
/// let mut animation = scene.compile([rise_in(dot), rise_in(label)]);
/// animation.sample(s(5));
/// assert_eq!(animation.get(dot).position, Point::new(0.0, 20.0));
/// assert_eq!(animation.get(label).opacity, 1.0);
/// ```
pub trait Transformable<M: Model2d = Canvas>:
    ObjectHandle<M>
{
    /// Marker for the position field.
    type Position: Field<Self::State, Point>;
    /// Marker for the position's x.
    type PositionX: Field<Self::State, f64>;
    /// Marker for the position's y.
    type PositionY: Field<Self::State, f64>;
    /// Marker for the scale field.
    type Scale: Field<Self::State, f64>;
    /// Marker for the rotation field.
    type Rotation: Field<Self::State, f64>;

    /// The position, in canvas units (y up).
    fn position(self) -> FieldOf<Self, M, Point, Self::Position> {
        Target::new(ObjectHandle::id(self))
    }
    /// The position's x.
    fn position_x(self) -> FieldOf<Self, M, f64, Self::PositionX> {
        Target::new(ObjectHandle::id(self))
    }
    /// The position's y (up is positive).
    fn position_y(self) -> FieldOf<Self, M, f64, Self::PositionY> {
        Target::new(ObjectHandle::id(self))
    }
    /// The uniform scale; 1 is normal size.
    fn scale(self) -> FieldOf<Self, M, f64, Self::Scale> {
        Target::new(ObjectHandle::id(self))
    }
    /// The rotation in radians, counterclockwise.
    fn rotation(self) -> FieldOf<Self, M, f64, Self::Rotation> {
        Target::new(ObjectHandle::id(self))
    }
    /// Move by `(x, y)` from wherever the object is when this starts.
    #[track_caller]
    fn shift(self, x: f64, y: f64) -> Animation<M> {
        Transformable::position(self).offset((x, y))
    }
    /// Move horizontally by `x`.
    #[track_caller]
    fn shift_x(self, x: f64) -> Animation<M> {
        Transformable::position_x(self).offset(x)
    }
    /// Move vertically by `y` (up is positive).
    #[track_caller]
    fn shift_y(self, y: f64) -> Animation<M> {
        Transformable::position_y(self).offset(y)
    }
    /// Scale by `factor` relative to the current scale.
    #[track_caller]
    fn scale_by(self, factor: f64) -> Animation<M> {
        Transformable::scale(self).multiply(factor)
    }
    /// Rotate by `angle` radians, counterclockwise.
    #[track_caller]
    fn rotate(self, angle: f64) -> Animation<M> {
        Transformable::rotation(self).offset(angle)
    }
}

/// Objects with a fill colour: closed shapes and text.
pub trait Fillable<M: Model2d = Canvas>: ObjectHandle<M> {
    /// Marker for the fill field.
    type Fill: Field<Self::State, Color>;

    /// The fill colour.
    fn fill(self) -> FieldOf<Self, M, Color, Self::Fill> {
        Target::new(ObjectHandle::id(self))
    }
    /// Animate the fill colour to `color`.
    #[track_caller]
    fn fill_to(self, color: impl Into<Color>) -> Animation<M> {
        Fillable::fill(self).to(color)
    }
    /// Change the fill colour instantly.
    #[track_caller]
    fn set_fill(self, color: impl Into<Color>) -> Animation<M> {
        Fillable::fill(self).set(color)
    }
}

/// Objects with an outline: shapes, lines and paths.
pub trait Strokable<M: Model2d = Canvas>: ObjectHandle<M> {
    /// Marker for the stroke colour field.
    type Stroke: Field<Self::State, Color>;
    /// Marker for the stroke width field.
    type StrokeWidth: Field<Self::State, f64>;

    /// The outline colour.
    fn stroke(self) -> FieldOf<Self, M, Color, Self::Stroke> {
        Target::new(ObjectHandle::id(self))
    }
    /// The outline width in canvas units.
    fn stroke_width(
        self,
    ) -> FieldOf<Self, M, f64, Self::StrokeWidth> {
        Target::new(ObjectHandle::id(self))
    }
}

/// Objects that can fade: every drawn object.
pub trait Fadable<M: Model2d = Canvas>: ObjectHandle<M> {
    /// Marker for the opacity field.
    type Opacity: Field<Self::State, f64>;

    /// 0 (invisible) to 1 (opaque).
    fn opacity(self) -> FieldOf<Self, M, f64, Self::Opacity> {
        Target::new(ObjectHandle::id(self))
    }
}

// Implements `ObjectHandle` and the listed traits for a handle type, using
// the field markers that `object!` declared in the same module. `id` is
// where the handle keeps its object ID.
macro_rules! handle_traits {
    (@ Transformable [$($generics:tt)*] $handle:ty) => {
        impl<M: crate::Model2d, $($generics)*> crate::Transformable<M> for $handle {
            type Position = fields::position::Marker;
            type PositionX = fields::position_x::Marker;
            type PositionY = fields::position_y::Marker;
            type Scale = fields::scale::Marker;
            type Rotation = fields::rotation::Marker;
        }
    };
    (@ Fillable [$($generics:tt)*] $handle:ty) => {
        impl<M: crate::Model2d, $($generics)*> crate::Fillable<M> for $handle {
            type Fill = fields::fill::Marker;
        }
    };
    (@ Strokable [$($generics:tt)*] $handle:ty) => {
        impl<M: crate::Model2d, $($generics)*> crate::Strokable<M> for $handle {
            type Stroke = fields::stroke::Marker;
            type StrokeWidth = fields::stroke_width::Marker;
        }
    };
    (@ Fadable [$($generics:tt)*] $handle:ty) => {
        impl<M: crate::Model2d, $($generics)*> crate::Fadable<M> for $handle {
            type Opacity = fields::opacity::Marker;
        }
    };
    (@ ObjectHandle [$($generics:tt)*] $handle:ty => $state:ty, $id:ty,
        $($field:ident).+) => {
        impl<M: crate::Model2d, $($generics)*> motiongfx_authoring::ObjectHandle<M> for $handle {
            type Id = $id;
            type State = $state;
            fn id(self) -> $id {
                self.$($field).+
            }
        }
    };
    (impl $generics:tt $handle:ty => $state:ty, $id:ty, id: $($field:ident).+;
        $($capability:ident),* $(,)?) => {
        crate::capability::handle_traits!(@ ObjectHandle $generics $handle => $state, $id,
            $($field).+);
        $(crate::capability::handle_traits!(@ $capability $generics $handle);)*
    };
    ($handle:ty => $state:ty, $id:ty; $($capability:ident),* $(,)?) => {
        crate::capability::handle_traits!(impl [] $handle => $state, $id, id: id;
            $($capability),*);
    };
}
pub(crate) use handle_traits;

// Stores objects of type `$state` in `Canvas::$storage`, indexed by
// `$id`, which makes them animatable in any world with a 2D model.
macro_rules! stored_in {
    ($state:ty, $id:ty, $storage:ident) => {
        impl<M: crate::Model2d>
            motiongfx_authoring::WorldObject<$id, M> for $state
        {
            fn get(
                world: &crate::World<M>,
                id: $id,
            ) -> Option<&Self> {
                world.model().canvas().$storage.get(id.0)
            }
            fn get_mut(
                world: &mut crate::World<M>,
                id: $id,
            ) -> Option<&mut Self> {
                world.model_mut().canvas_mut().$storage.get_mut(id.0)
            }
        }
    };
}
pub(crate) use stored_in;
