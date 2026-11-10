//! Compound position handles retain whole-point operations and expose axes.

use crate::Point;
use motiongfx::prelude::Path;
use motiongfx_authoring::Property;
use std::ops::Deref;

pub struct Position<I, S> {
    whole: Property<I, S, Point>,
    pub x: Property<I, S, f64>,
    pub y: Property<I, S, f64>,
}

impl<I: Copy, S> Copy for Position<I, S> {}
impl<I: Copy, S> Clone for Position<I, S> {
    fn clone(&self) -> Self {
        *self
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
        }
    }
}

// Allows position.to(...) / position.offset(...) to use the same Property
// implementation as every other target, without duplicating those methods.
impl<I, S> Deref for Position<I, S> {
    type Target = Property<I, S, Point>;
    fn deref(&self) -> &Self::Target {
        &self.whole
    }
}
