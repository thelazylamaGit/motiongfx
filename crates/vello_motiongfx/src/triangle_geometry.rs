//! Triangle outlines, cached per triangle.
use motiongfx_2d::Geometry;
/// One triangle's outline, rebuilt only when its size, angle or radius
/// changes.
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
