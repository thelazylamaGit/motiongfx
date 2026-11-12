//! Reuse the existing Peniko tracer, caching its output outside sampled state.

use crate::Path;
use peniko_motiongfx::trace::Trace;
use std::sync::Arc;
use vello::kurbo::{Affine, BezPath, Stroke};

pub(crate) struct PathGeometry {
    source: Arc<BezPath>,
    range: (f64, f64),
    pub(crate) visible: BezPath,
    #[cfg(test)]
    pub(crate) rebuilds: usize,
}

impl PathGeometry {
    pub(crate) fn new(path: &Path) -> Self {
        Self {
            source: path.geometry.clone(),
            range: (path.start, path.end),
            visible: path
                .geometry
                .trace_range(path.start as f32, path.end as f32),
            #[cfg(test)]
            rebuilds: 1,
        }
    }

    pub(crate) fn render(
        &mut self,
        path: &Path,
        scene: &mut vello::Scene,
        screen: Affine,
    ) {
        if !Arc::ptr_eq(&self.source, &path.geometry)
            || self.range != (path.start, path.end)
        {
            *self = Self {
                #[cfg(test)]
                rebuilds: self.rebuilds + 1,
                ..Self::new(path)
            };
        }
        let opacity = path.opacity.clamp(0.0, 1.0) as f32;
        if opacity == 0.0
            || path.stroke_width <= 0.0
            || self.visible.is_empty()
        {
            return;
        }
        let xf = screen
            * Affine::translate((path.position.x, -path.position.y))
            * Affine::rotate(-path.rotation)
            * Affine::scale_non_uniform(path.scale, -path.scale);
        scene.stroke(
            &Stroke::new(path.stroke_width)
                .with_caps(path.line_cap)
                .with_join(path.line_join),
            xf,
            path.stroke
                .with_alpha(path.stroke.components[3] * opacity),
            None,
            &self.visible,
        );
    }
}
