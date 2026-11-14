use motiongfx_text::{
    FontSystem, LayoutCache, LayoutStyle, TextRange,
};

#[test]
fn embedded_font_shapes_lowercase_unicode_and_ligatures() {
    let mut fonts = FontSystem::default();
    let text = "office café π";
    let layout = fonts.layout(text.into(), &LayoutStyle::default());
    let glyphs: Vec<_> =
        layout.runs.iter().flat_map(|r| &r.glyphs).collect();
    assert!(layout.size.width > 0.0 && layout.size.height > 0.0);
    assert!(
        glyphs.iter().all(
            |g| g.id != 0 && text.get(g.source.clone()).is_some()
        )
    );
    let ligature = TextRange::find(text, "fi", 0).unwrap();
    let bounds = layout.selection_bounds(&ligature).unwrap();
    assert!(bounds.width() > 0.0 && bounds.height() > 0.0);
    // Selecting part of a ligature must include its whole drawable cluster.
    let part = TextRange::find(text, "i", 0).unwrap();
    assert_eq!(layout.selection_bounds(&part), Some(bounds));
}

#[test]
fn ranges_validate_utf8_and_remain_bound_to_their_source() {
    assert!(TextRange::new("café", 3..4).is_err());
    assert!(TextRange::new("café", 5..6).is_err());
    assert!(TextRange::find("hello", "", 0).is_err());
    assert!(TextRange::find("hello", "missing", 0).is_err());
    let range = TextRange::find("one one", "one", 1).unwrap();
    assert_eq!(range.range(), 4..7);
    assert!(range.overlaps("one one", &(5..6)));
    assert!(!range.overlaps("two one", &(5..6)));
}

#[test]
fn wrapping_preserves_source_bounds_and_font_fallback() {
    let mut fonts = FontSystem::default();
    let style = LayoutStyle::default();
    let wide = fonts.layout("hello world hello world".into(), &style);
    let wrapped = fonts.layout(
        "hello world hello world".into(),
        &LayoutStyle {
            wrap_width: 100.0,
            ..style.clone()
        },
    );
    assert!(wrapped.size.height > wide.size.height);
    assert!(
        wrapped
            .selection_bounds(
                &TextRange::find(
                    "hello world hello world",
                    "world",
                    1
                )
                .unwrap()
            )
            .unwrap()
            .y0
            > 0.0
    );
    let fallback = fonts.layout(
        "hello world hello world".into(),
        &LayoutStyle {
            family: "Unregistered family".into(),
            ..style
        },
    );
    assert_eq!(fallback.size, wide.size);
}

#[test]
fn cache_invalidates_only_for_layout_inputs_or_registered_fonts() {
    let mut fonts = FontSystem::default();
    let mut cache = LayoutCache::default();
    let mut style = LayoutStyle::default();
    cache.get(&mut fonts, "hello".into(), style.clone());
    cache.get(&mut fonts, "hello".into(), style.clone());
    assert_eq!(cache.builds(), 1);
    style.size = 48.0;
    cache.get(&mut fonts, "hello".into(), style.clone());
    assert_eq!(cache.builds(), 2);
    cache.get(&mut fonts, "changed".into(), style.clone());
    assert_eq!(cache.builds(), 3);
    let revision = fonts.revision();
    assert!(!fonts.register_font(b"invalid font"));
    assert_eq!(fonts.revision(), revision);
    assert!(fonts.register_font(include_bytes!(
        "../fonts/FiraSans-Regular.ttf"
    )));
    cache.get(&mut fonts, "changed".into(), style);
    assert_eq!(cache.builds(), 4);
}

#[test]
fn invalid_animated_layout_values_produce_no_glyphs() {
    let mut fonts = FontSystem::default();
    for size in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let layout = fonts.layout(
            "text".into(),
            &LayoutStyle {
                size,
                ..Default::default()
            },
        );
        assert!(layout.runs.is_empty());
    }
}

#[test]
fn tabular_numbers_give_digits_equal_advances() {
    let mut fonts = FontSystem::default();
    let mut width = |text: &str, tabular_numbers| {
        let style = LayoutStyle {
            tabular_numbers,
            ..LayoutStyle::default()
        };
        fonts.layout(text.into(), &style).size.width
    };
    // Embedded Fira Sans has proportional default figures: "1" is narrow.
    assert!(width("111", false) < width("000", false));
    assert_eq!(width("111", true), width("000", true));
}
