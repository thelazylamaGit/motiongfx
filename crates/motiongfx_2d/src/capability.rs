//! Traits for what several kinds of objects share, such as a position or
//! a fill. A [`Handle`] to an object with one gets its methods, and
//! generic code can require it of `T` in `Handle<T>`.
use crate::{Canvas, Color, Handle, Model2d, Object2d, Point};
use motiongfx_authoring::{Animation, Field, Target, WorldObject};

/// The field of `T` identified by marker `F`.
type FieldOf<T, V, F> = Target<<T as Object2d>::Id, T, V, F>;

/// Objects with a position, scale and rotation: shapes, paths, text and
/// the camera. Their handles have `position()`, `shift(..)`, `rotate(..)`
/// and so on; see [`Handle`] for a generic example.
///
/// The `WorldObject` bound for [`Canvas`] lets generic code over
/// `Handle<T>` animate these fields in ordinary 2D scenes; for a
/// composite model `M`, also require `T: WorldObject<T::Id, M>`.
pub trait Transformable:
    Object2d + WorldObject<<Self as Object2d>::Id, Canvas>
{
    /// Marker for the position field.
    type Position: Field<Self, Point>;
    /// Marker for the position's x.
    type PositionX: Field<Self, f64>;
    /// Marker for the position's y.
    type PositionY: Field<Self, f64>;
    /// Marker for the scale field.
    type Scale: Field<Self, f64>;
    /// Marker for the rotation field.
    type Rotation: Field<Self, f64>;
}

/// Methods of every [`Transformable`] object's handle.
impl<T: Transformable> Handle<T> {
    /// The position, in canvas units (y up).
    pub fn position(self) -> FieldOf<T, Point, T::Position> {
        Target::new(self.id)
    }
    /// The position's x.
    pub fn position_x(self) -> FieldOf<T, f64, T::PositionX> {
        Target::new(self.id)
    }
    /// The position's y (up is positive).
    pub fn position_y(self) -> FieldOf<T, f64, T::PositionY> {
        Target::new(self.id)
    }
    /// The uniform scale; 1 is normal size.
    pub fn scale(self) -> FieldOf<T, f64, T::Scale> {
        Target::new(self.id)
    }
    /// The rotation in radians, counterclockwise.
    pub fn rotation(self) -> FieldOf<T, f64, T::Rotation> {
        Target::new(self.id)
    }
    /// Move by `(x, y)` from wherever the object is when this starts.
    #[track_caller]
    pub fn shift<M: Model2d>(self, x: f64, y: f64) -> Animation<M>
    where
        T: WorldObject<T::Id, M>,
    {
        self.position().offset((x, y))
    }
    /// Move horizontally by `x`.
    #[track_caller]
    pub fn shift_x<M: Model2d>(self, x: f64) -> Animation<M>
    where
        T: WorldObject<T::Id, M>,
    {
        self.position_x().offset(x)
    }
    /// Move vertically by `y` (up is positive).
    #[track_caller]
    pub fn shift_y<M: Model2d>(self, y: f64) -> Animation<M>
    where
        T: WorldObject<T::Id, M>,
    {
        self.position_y().offset(y)
    }
    /// Scale by `factor` relative to the current scale.
    #[track_caller]
    pub fn scale_by<M: Model2d>(self, factor: f64) -> Animation<M>
    where
        T: WorldObject<T::Id, M>,
    {
        self.scale().multiply(factor)
    }
    /// Rotate by `angle` radians, counterclockwise.
    #[track_caller]
    pub fn rotate<M: Model2d>(self, angle: f64) -> Animation<M>
    where
        T: WorldObject<T::Id, M>,
    {
        self.rotation().offset(angle)
    }
}

/// Objects with a fill colour: closed shapes and text. Their handles have
/// `fill()`, `fill_to(..)` and `set_fill(..)`.
pub trait Fillable:
    Object2d + WorldObject<<Self as Object2d>::Id, Canvas>
{
    /// Marker for the fill field.
    type Fill: Field<Self, Color>;
}

/// Methods of every [`Fillable`] object's handle.
impl<T: Fillable> Handle<T> {
    /// The fill colour.
    pub fn fill(self) -> FieldOf<T, Color, T::Fill> {
        Target::new(self.id)
    }
    /// Animate the fill colour to `color`.
    #[track_caller]
    pub fn fill_to<M: Model2d>(
        self,
        color: impl Into<Color>,
    ) -> Animation<M>
    where
        T: WorldObject<T::Id, M>,
    {
        self.fill().to(color)
    }
    /// Change the fill colour instantly.
    #[track_caller]
    pub fn set_fill<M: Model2d>(
        self,
        color: impl Into<Color>,
    ) -> Animation<M>
    where
        T: WorldObject<T::Id, M>,
    {
        self.fill().set(color)
    }
}

/// Objects with an outline: shapes, lines and paths. Their handles have
/// `stroke()` and `stroke_width()`.
pub trait Strokable:
    Object2d + WorldObject<<Self as Object2d>::Id, Canvas>
{
    /// Marker for the stroke colour field.
    type Stroke: Field<Self, Color>;
    /// Marker for the stroke width field.
    type StrokeWidth: Field<Self, f64>;
}

/// Methods of every [`Strokable`] object's handle.
impl<T: Strokable> Handle<T> {
    /// The outline colour.
    pub fn stroke(self) -> FieldOf<T, Color, T::Stroke> {
        Target::new(self.id)
    }
    /// The outline width in canvas units.
    pub fn stroke_width(self) -> FieldOf<T, f64, T::StrokeWidth> {
        Target::new(self.id)
    }
}

/// Objects that can fade: every drawn object. Their handles have
/// `opacity()`.
pub trait Fadable:
    Object2d + WorldObject<<Self as Object2d>::Id, Canvas>
{
    /// Marker for the opacity field.
    type Opacity: Field<Self, f64>;
}

/// Methods of every [`Fadable`] object's handle.
impl<T: Fadable> Handle<T> {
    /// 0 (invisible) to 1 (opaque).
    pub fn opacity(self) -> FieldOf<T, f64, T::Opacity> {
        Target::new(self.id)
    }
}

// Implements the listed traits for an object type, using the field
// markers that `object!` declared in the same module.
macro_rules! capabilities {
    (@ Transformable $state:ty) => {
        impl crate::Transformable for $state {
            type Position = fields::position::Marker;
            type PositionX = fields::position_x::Marker;
            type PositionY = fields::position_y::Marker;
            type Scale = fields::scale::Marker;
            type Rotation = fields::rotation::Marker;
        }
    };
    (@ Fillable $state:ty) => {
        impl crate::Fillable for $state {
            type Fill = fields::fill::Marker;
        }
    };
    (@ Strokable $state:ty) => {
        impl crate::Strokable for $state {
            type Stroke = fields::stroke::Marker;
            type StrokeWidth = fields::stroke_width::Marker;
        }
    };
    (@ Fadable $state:ty) => {
        impl crate::Fadable for $state {
            type Opacity = fields::opacity::Marker;
        }
    };
    ($state:ty; $($capability:ident),* $(,)?) => {
        $(crate::capability::capabilities!(@ $capability $state);)*
    };
}
pub(crate) use capabilities;

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
