//! Renderer-owned rounded triangle cache.
use motiongfx_2d::Geometry;
/// Kept outside sampled Triangle state, like the existing path/text caches.
/// Only size, angle and radius change local geometry; styles/transforms don't.
pub struct CachedTriangle {
    key: [f64; 3],
    pub geometry: Geometry,
    #[cfg(test)]
    pub rebuilds: usize,
}
impl CachedTriangle {
    pub fn new(triangle: &crate::Triangle) -> Self {
        Self {
            key: [triangle.size, triangle.angle, triangle.radius],
            geometry: triangle.geometry(),
            #[cfg(test)]
            rebuilds: 1,
        }
    }
    pub fn update(&mut self, triangle: &crate::Triangle) {
        if self.key
            != [triangle.size, triangle.angle, triangle.radius]
        {
            *self = Self {
                #[cfg(test)]
                rebuilds: self.rebuilds + 1,
                ..Self::new(triangle)
            };
        }
    }
}
