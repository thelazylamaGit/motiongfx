//! Spring settings, re-exported from core. Pass one to
//! [`Animation::spring`](crate::Animation::spring) to animate a change with
//! a spring instead of a tween.
pub use motiongfx::spring::{
    Spring, SpringCurve, SpringError, SpringSample, SpringValue,
};
