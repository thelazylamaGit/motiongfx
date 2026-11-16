//! [`Handle`]: a typed reference to one object in a scene.
use motiongfx::prelude::ThreadSafe;
use motiongfx::subject::SubjectId;
use motiongfx_authoring::{ObjectHandle, WorldObject};

/// A 2D object type that a scene stores, such as [`Circle`](crate::Circle).
/// Its [`Handle`]s identify single objects of the type.
pub trait Object2d: Clone + ThreadSafe {
    /// The ID type for objects of this type, such as `CircleId`.
    type Id: SubjectId;
}

/// One object in a scene, as returned by `scene.add`: a `Handle<Circle>`
/// for a circle, and so on. Its methods return the object's fields to
/// animate (`.to(..)`) or for other objects to follow.
///
/// Which methods a handle has depends on its object: every
/// [`Transformable`](crate::Transformable) object's handle has
/// `position()` and `shift(..)`, a circle's also has `radius()`. Generic
/// code can require those traits of `T`:
///
/// ```
/// use motiongfx::prelude::s;
/// use motiongfx_2d::prelude::*;
///
/// fn rise_in<T: Transformable + Fadable>(object: Handle<T>) -> Animation {
///     all([object.shift_y(20.0), object.opacity().to(1.0)])
/// }
///
/// let mut scene = Scene::new();
/// let dot = scene.add(Circle::new(10.0).opacity(0.0));
/// let label = scene.add(Text::new("Hi").opacity(0.0));
/// let mut animation = scene.compile([rise_in(dot), rise_in(label)]);
/// animation.sample(s(5));
/// assert_eq!(animation.get(dot).transform.position, Point::new(0.0, 20.0));
/// assert_eq!(animation.get(label).opacity, 1.0);
/// ```
pub struct Handle<T: Object2d> {
    pub(crate) id: T::Id,
}

impl<T: Object2d> Handle<T> {
    pub(crate) const fn new(id: T::Id) -> Self {
        Self { id }
    }
    /// The object's ID.
    pub fn id(self) -> T::Id {
        self.id
    }
}

// Implemented by hand: deriving would require `T: Copy`, `T: PartialEq` and
// so on, but only the ID is stored and compared.
impl<T: Object2d> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Object2d> Copy for Handle<T> {}
impl<T: Object2d> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<T: Object2d> Eq for Handle<T> {}
impl<T: Object2d> PartialOrd for Handle<T> {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T: Object2d> Ord for Handle<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}
impl<T: Object2d> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
/// Shows the object's ID, such as `Handle(CircleId(3))`.
impl<T: Object2d> std::fmt::Debug for Handle<T> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_tuple("Handle").field(&self.id).finish()
    }
}

impl<T: Object2d, M> ObjectHandle<M> for Handle<T>
where
    T: WorldObject<T::Id, M>,
{
    type Id = T::Id;
    type State = T;
    fn id(self) -> T::Id {
        self.id
    }
}
