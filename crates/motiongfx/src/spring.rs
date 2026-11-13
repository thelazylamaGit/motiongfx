//! Core spring actions. Finite analytical clips use existing track ordering.
use crate::{action::InterpActionBuilder, track::TrackFragment};
pub use motiongfx_interp::spring::{
    Spring, SpringCurve, SpringError, SpringSample, SpringValue,
};

/// A spring has no `.with_ease` or explicit-duration `.play(duration)` method.
/// Use `try_play` for checked preparation, or `play` for the panicking convenience.
pub struct SpringActionBuilder<'a, T> {
    pub(crate) inner: InterpActionBuilder<'a, T>,
    pub(crate) spring: Spring,
}
impl<T> SpringActionBuilder<'_, T> {
    pub fn try_play(self) -> Result<TrackFragment, SpringError> {
        let curve = self.spring.prepare()?;
        Ok(self.inner.play_spring(curve))
    }
    #[track_caller]
    pub fn play(self) -> TrackFragment {
        self.try_play().expect("invalid spring settings")
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SpringStorage(pub SpringCurve);
