//! Cached output of the temporary text renderer, separate from sampled Text.

use crate::{Text, TextAlign, lettering};
use std::sync::Arc;
use vello::kurbo::{Affine, BezPath, Size};
use vello::peniko::Fill;

pub(crate) struct TextLayout {
    content: Arc<str>,
    path: BezPath,
    size: Size,
    #[cfg(test)]
    pub(crate) rebuilds: usize,
}

impl TextLayout {
    pub(crate) fn new(text: &Text) -> Self {
        let (path, size) = lettering::layout(&text.content);
        Self {
            content: text.content.clone(),
            path,
            size,
            #[cfg(test)]
            rebuilds: 1,
        }
    }

    pub(crate) fn render(
        &mut self,
        text: &Text,
        scene: &mut vello::Scene,
        screen: Affine,
    ) {
        if self.content != text.content {
            *self = Self {
                #[cfg(test)]
                rebuilds: self.rebuilds + 1,
                ..Self::new(text)
            };
        }
        let opacity = text.opacity.clamp(0.0, 1.0);
        if opacity == 0.0
            || text.font_size <= 0.0
            || text.scale == 0.0
        {
            return;
        }
        let anchor = match text.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => self.size.width / 2.0,
            TextAlign::Right => self.size.width,
        };
        // Lettering paths use y-down coordinates to keep glyphs upright.
        let xf = screen
            * Affine::translate((text.position.x, -text.position.y))
            * Affine::rotate(-text.rotation)
            * Affine::scale(text.scale * text.font_size / 7.0)
            * Affine::translate((-anchor, 0.0));
        let alpha = text.fill.components[3] * opacity as f32;
        scene.fill(
            Fill::NonZero,
            xf,
            text.fill.with_alpha(alpha),
            None,
            &self.path,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, colors};

    #[test]
    fn geometry_is_reused_for_transform_style_and_size_changes() {
        let mut text = Text::new("Abc\n123");
        let mut layout = TextLayout::new(&text);
        let mut scene = vello::Scene::new();
        layout.render(&text, &mut scene, Affine::IDENTITY);
        assert!(!scene.encoding().path_data.is_empty());
        assert_eq!(layout.rebuilds, 1);
        assert_eq!(layout.size, Size::new(17.0, 16.0));
        text.position = Point::new(40.0, 20.0);
        text.font_size = 48.0;
        text.scale = 2.0;
        text.rotation = 0.5;
        text.fill = colors::BLUE;
        text.opacity = 0.25;
        text.align = TextAlign::Center;
        scene.reset();
        layout.render(&text, &mut scene, Affine::IDENTITY);
        assert_eq!(layout.rebuilds, 1);
        text.content = "Changed".into();
        scene.reset();
        layout.render(&text, &mut scene, Affine::IDENTITY);
        assert_eq!(layout.rebuilds, 2);
        text.content = "Abc\n123".into();
        scene.reset();
        layout.render(&text, &mut scene, Affine::IDENTITY);
        assert_eq!(layout.rebuilds, 3);
    }
}
