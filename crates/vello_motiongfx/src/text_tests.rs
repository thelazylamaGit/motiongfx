use super::*;
use text_renderer::TextLayout;

#[test]
fn paint_and_transforms_reuse_layout_but_font_size_reshapes() {
    let mut text: Object<Text> = Text::new("café office");
    let mut cache = TextLayout::default();
    let mut fonts = None;
    let mut frame = vello::Scene::new();
    cache.render(
        &text,
        &mut frame,
        Affine::IDENTITY,
        &mut fonts,
        &[],
    );
    assert!(!frame.encoding().resources.glyphs.is_empty());
    text.fill = colors::RED;
    text.opacity = 0.5;
    text.position = Point::new(20.0, 30.0);
    text.scale = 1.5;
    text.rotation = 0.2;
    frame.reset();
    cache.render(
        &text,
        &mut frame,
        Affine::IDENTITY,
        &mut fonts,
        &[],
    );
    assert_eq!(cache.fonts.builds(), 1);
    assert_eq!(cache.mappings, 1);
    text.font_size = 48.0;
    frame.reset();
    cache.render(
        &text,
        &mut frame,
        Affine::IDENTITY,
        &mut fonts,
        &[],
    );
    assert_eq!(cache.fonts.builds(), 2);
    assert_eq!(cache.mappings, 2);
}

#[test]
fn word_paint_seeks_and_source_changes_do_not_use_stale_ranges() {
    let mut scene = Scene::new();
    let text = scene.add(Text::new("hello world"));
    let word = text.select("world");
    let mut animation = scene.compile([
        all([
            word.fill().to(colors::RED).over(s(1)),
            word.opacity().to(0.5).over(s(1)),
        ]),
        text.set_content("different"),
        wait(s(1)),
    ]);
    let mut renderer = Renderer::default();
    let mut frame = vello::Scene::new();
    let size = Size::new(400.0, 200.0);
    let mut initial_draw = Vec::new();
    for (time, batches, builds) in
        [(0, 1, 1), (500, 2, 1), (2000, 1, 2), (0, 1, 3)]
    {
        animation.sample(ms(time));
        frame.reset();
        renderer.render(&animation, &mut frame, size, size, 1.0);
        assert_eq!(
            frame.encoding().resources.glyph_runs.len(),
            batches
        );
        assert_eq!(renderer.text_layouts[0].fonts.builds(), builds);
        assert_eq!(renderer.text_layouts[0].mappings, builds);
        if initial_draw.is_empty() {
            initial_draw = frame.encoding().draw_data.clone();
        } else if time == 0 {
            assert_eq!(frame.encoding().draw_data, initial_draw);
        }
    }
    // Weight is a typed, continuously animatable numeric value.
    let mut scene = Scene::new();
    let text = scene.add(Text::new("weight").font_weight(400.0));
    let mut animation = scene.compile([text
        .font_weight()
        .to(700.0)
        .over(s(1))
        .ease(ease::linear)]);
    animation.sample(ms(500));
    assert_eq!(animation.text(text).font_weight.value(), 550.0);
}

#[test]
fn a_reused_renderer_remaps_selection_ranges_without_reshaping() {
    let mut renderer = Renderer::default();
    let size = Size::new(400.0, 200.0);
    for (needle, first_batch_len, mappings) in
        [("hello", 5, 1), ("world", 6, 2)]
    {
        let mut scene = Scene::new();
        let title = scene.add(Text::new("hello world"));
        let animation = scene.compile([
            title.select(needle).fill().set(colors::RED),
            wait(s(1)),
        ]);
        let mut frame = vello::Scene::new();
        renderer.render(&animation, &mut frame, size, size, 1.0);
        assert_eq!(renderer.text_layouts[0].fonts.builds(), 1);
        assert_eq!(renderer.text_layouts[0].mappings, mappings);
        assert_eq!(
            frame.encoding().resources.glyph_runs[0].glyphs.len(),
            first_batch_len
        );
    }
}
