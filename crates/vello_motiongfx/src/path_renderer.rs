//! Path drawing: the visible part of each path, cached until the path's
//! geometry or visible range changes.

use crate::outline::Outline;
use motiongfx_2d::Path;
use peniko_motiongfx::trace::Trace;
use std::sync::Arc;
use vello::kurbo::{Affine, BezPath};

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
        let Some((style, color)) = path.outline(opacity) else {
            return;
        };
        if self.visible.is_empty() {
            return;
        }
        let xf = screen
            * Affine::translate((
                path.transform.position.x,
                -path.transform.position.y,
            ))
            * Affine::rotate(-path.transform.rotation)
            * Affine::scale_non_uniform(
                path.transform.scale,
                -path.transform.scale,
            );
        scene.stroke(&style, xf, color, None, &self.visible);
    }
}
