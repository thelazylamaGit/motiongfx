//! Partial styles must preserve normal setter, binding and seeking semantics.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn partial_styles_layer_without_resetting_unspecified_properties_or_retaining_the_preset()
 {
    let base =
        TextStyle::new().font_size(40.0).align(TextAlign::Center);
    let mut emphasis = TextStyle::new().fill(colors::RED);
    let descriptor = Text::new("Label")
        .position((10.0, 20.0))
        .opacity(0.5)
        .font_size(12.0)
        .style(&base)
        .style(&emphasis)
        .font_size(30.0)
        .style(&TextStyle::new()); // An empty style has no defaults to apply.
    emphasis = emphasis.fill(colors::BLUE);
    let mut scene = Scene::new();
    let a = scene.add(descriptor);
    let b =
        scene.add(Text::new("Other").style(&base).style(&emphasis));
    let animation = scene.compile([wait(s(1))]);
    let a = animation.text(a);
    assert_eq!(a.content.as_ref(), "Label");
    assert_eq!(a.position, Point::new(10.0, 20.0));
    assert_eq!(a.opacity, 0.5);
    assert_eq!(a.font_size, 30.0);
    assert!(a.align == TextAlign::Center);
    assert_eq!(a.fill, colors::RED);
    assert_eq!(animation.text(b).fill, colors::BLUE);
    assert_eq!(animation.text(b).font_size, 40.0);
}

#[test]
fn style_signals_and_computations_share_existing_caches_and_rewind() {
    let size = signal(24.0);
    let alignment = signal(TextAlign::Left);
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let opacity = size.map(move |size| {
        counter.fetch_add(1, Ordering::Relaxed);
        size / 48.0
    });
    let mut scene = Scene::new();
    // Drop the style before finalisation: objects must retain the inputs, not
    // references to this local preset, and share the computed's identity.
    let (a, b) = {
        let labels = TextStyle::new()
            .font_size(&size)
            .opacity(&opacity)
            .align(&alignment);
        (
            scene.add(Text::new("A").style(&labels)),
            scene.add(Text::new("B").style(&labels)),
        )
    };
    let mut animation = scene.compile([
        size.to(48.0).ease(ease::linear),
        alignment.set(TextAlign::Center),
        wait(s(1)),
    ]);
    for (time, size, align, calls_expected) in [
        (0, 24.0, TextAlign::Left, 1),
        (500, 36.0, TextAlign::Left, 2),
        (500, 36.0, TextAlign::Left, 2),
        (1500, 48.0, TextAlign::Center, 3),
        (0, 24.0, TextAlign::Left, 4),
    ] {
        animation.sample(ms(time));
        for text in [a, b] {
            let text = animation.text(text);
            assert_eq!(text.font_size, size);
            assert_eq!(text.opacity, size / 48.0);
            assert!(text.align == align);
        }
        assert_eq!(calls.load(Ordering::Relaxed), calls_expected);
    }
}

#[test]
fn later_style_or_direct_setter_removes_obsolete_reactive_wiring() {
    let size = signal(24.0);
    let dynamic = TextStyle::new().font_size(&size);
    let fixed = TextStyle::new().font_size(30.0);
    let mut scene = Scene::new();
    let a = scene.add(Text::new("A").style(&dynamic).style(&fixed));
    let b = scene.add(Text::new("B").style(&dynamic).font_size(30.0));
    // Residual bindings would reject these animations as conflicting writers.
    let mut animation = scene
        .try_compile([all([
            a.font_size().to(40.0).ease(ease::linear),
            b.font_size().to(40.0).ease(ease::linear),
        ])])
        .unwrap();
    for (time, size) in [(500, 35.0), (1000, 40.0), (0, 30.0)] {
        animation.sample(ms(time));
        assert_eq!(animation.text(a).font_size, size);
        assert_eq!(animation.text(b).font_size, size);
    }
}
