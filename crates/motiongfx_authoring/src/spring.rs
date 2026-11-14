//! Spring settings. Any animation takes a spring through
//! [`Animation::spring`](crate::Animation::spring); it lowers to the core spring
//! action primitive with the property's own interpolation.
pub use motiongfx::spring::{
    Spring, SpringCurve, SpringError, SpringSample, SpringValue,
};
