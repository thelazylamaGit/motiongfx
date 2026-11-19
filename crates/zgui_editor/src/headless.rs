//! The timeline driven the way a person uses it, with no window and no
//! graphics device: the real document, style sheets, layout, event
//! routing and frame clock over zgui's headless platform, with a renderer
//! that draws nothing and a clock the tests move by hand.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use vello_winit_motiongfx::PlaybackConfig;
use zgui::geom::{Css, CssPx, Device, DevicePx, Point, Size};
use zgui::platform::{Surface, SurfaceEvent};
use zgui::prelude::*;
use zgui::render::{RenderTarget, Renderer};
use zgui::runtime::{AppError, Runtime};
use zgui::vocab::{
    KeyEvent, KeyState, PointerAction, PointerEvent, Timestamp,
};
use zgui_platform_headless::Harness;
use zgui_ui_tokens::prelude::ColorScheme;

use crate::clock::Clock;
use crate::controls::{self, Sound, SoundStatus};
use crate::editor::workspace;
use crate::theme::{self, EditorThemeProps};
use crate::timeline::{self, Viewport};

thread_local! {
    /// The track the timeline under test built, so a test can aim the
    /// pointer at it rather than at its own arithmetic.
    pub(crate) static TRACK: Cell<Option<NodeRef>> = const { Cell::new(None) };
    /// The play button, for a test that clicks it.
    pub(crate) static PLAY: Cell<Option<NodeRef>> = const { Cell::new(None) };
}

/// What the view builds and the rig takes out once it is up.
type Built = (Clock, RwSignal<Viewport, LocalStorage>);

/// One refresh at 60 Hz.
const REFRESH: Duration = Duration::from_micros(16_667);

/// What the tests reach into the running timeline for.
#[derive(Clone)]
struct Parts {
    clock: Clock,
    viewport: RwSignal<Viewport, LocalStorage>,
    track: NodeRef,
}

/// A timeline for a scene `duration` long, open in a headless window.
struct Rig {
    harness: Harness<Runtime>,
    parts: Parts,
}

impl Rig {
    fn open(duration: Duration, width: f32) -> Self {
        let built: Rc<RefCell<Option<Built>>> = Rc::default();
        let slot = Rc::clone(&built);
        let handler = zgui::app()
            .with_title("timeline")
            .with_size(width, timeline::HEIGHT as f32)
            .with_stylesheet(theme::SHEET)
            .with_stylesheet(timeline::SHEET)
            .with_stylesheet(controls::SHEET)
            .with_renderer(Box::new(
                |_surface: &Arc<dyn Surface>, target| {
                    let mut renderer = Nothing::default();
                    renderer.configure(target);
                    Ok::<_, AppError>(
                        Box::new(renderer) as Box<dyn Renderer>
                    )
                },
            ))
            .into_handler(move || {
                let clock = Clock::new(PlaybackConfig::new(duration));
                // Paused at the start, as a person scrubbing would be.
                clock.toggle();
                let viewport =
                    RwSignal::new_local(Viewport::whole(duration));
                let sound = Sound::new(SoundStatus::None, &clock);
                *slot.borrow_mut() = Some((clock.clone(), viewport));
                // The real window, keys and all, with an empty box where
                // the GPU preview would be.
                let editor = workspace(
                    clock,
                    viewport,
                    sound,
                    view! { box(class = "preview") {} },
                );
                view! {
                    EditorTheme(scheme = ColorScheme::Dark) {
                        {editor}
                    }
                }
            })
            .expect("the application assembles");
        let mut harness = Harness::new(handler.into_runtime());
        harness.deliver_to_first(SurfaceEvent::Resized(Size::<
            DevicePx,
            Device,
        >::new(
            DevicePx(width),
            DevicePx(timeline::HEIGHT as f32),
        )));
        harness.settle(64);
        let (clock, viewport) = built
            .borrow_mut()
            .take()
            .expect("the timeline was built");
        let track =
            TRACK.get().expect("the timeline recorded its track");
        Self {
            harness,
            parts: Parts {
                clock,
                viewport,
                track,
            },
        }
    }

    fn view(&self) -> Viewport {
        self.parts.viewport.get_untracked()
    }

    fn time(&self) -> f64 {
        self.parts.clock.state.get_untracked().time.as_secs_f64()
    }

    /// Zoom in `factor` times, keeping `anchor` of the width in place.
    fn zoom(&mut self, factor: f64, anchor: f64) {
        self.parts
            .viewport
            .update(|view| *view = view.zoomed(factor, anchor));
        self.harness.settle(16);
    }

    /// Where `fraction` of the way along the track is, `dx` CSS pixels
    /// further, at the lane's height.
    fn on_track(&self, fraction: f64, dx: f32) -> Point<CssPx, Css> {
        let bounds = self
            .parts
            .track
            .window_bounds()
            .expect("the track is laid out");
        let scale = self.parts.track.scale();
        let x =
            bounds.origin.x.0 + bounds.size.width.0 * fraction as f32;
        let y = bounds.origin.y.0 + bounds.size.height.0 * 0.6;
        Point::new(CssPx(x / scale + dx), CssPx(y / scale))
    }

    fn pointer(
        &mut self,
        action: PointerAction,
        at: Point<CssPx, Css>,
    ) {
        let mut event = PointerEvent::mouse(at);
        if matches!(
            action,
            PointerAction::Pressed | PointerAction::Released
        ) {
            event = event.with_button(PointerButton::Primary);
        }
        self.harness.deliver_to_first(SurfaceEvent::Pointer {
            action,
            event,
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
        self.harness.settle(16);
    }

    /// Press and release the named `key`.
    fn key(&mut self, key: NamedKey, code: KeyCode) {
        for state in [KeyState::Pressed, KeyState::Released] {
            self.harness.deliver_to_first(SurfaceEvent::Key {
                state,
                event: KeyEvent::named(key, PhysicalKey::Code(code)),
                modifiers: Modifiers::NONE,
                timestamp: Timestamp::ORIGIN,
            });
            self.harness.settle(16);
        }
    }

    /// Click the middle of `node`.
    fn click(&mut self, node: NodeRef) {
        let bounds = node.window_bounds().expect("it is laid out");
        let scale = node.scale();
        let at = Point::new(
            CssPx(
                (bounds.origin.x.0 + bounds.size.width.0 / 2.0)
                    / scale,
            ),
            CssPx(
                (bounds.origin.y.0 + bounds.size.height.0 / 2.0)
                    / scale,
            ),
        );
        self.pointer(PointerAction::Moved, at);
        self.pointer(PointerAction::Pressed, at);
        self.pointer(PointerAction::Released, at);
    }

    /// Run `count` refreshes of the frame clock.
    fn frames(&mut self, count: u32) {
        for _ in 0..count {
            self.harness.advance(REFRESH);
            self.harness.pump();
        }
    }
}

#[test]
fn scrubbing_follows_the_pointer_inside_a_zoomed_view() {
    let mut rig = Rig::open(Duration::from_secs(10), 900.0);
    rig.zoom(5.0, 0.0);
    let view = rig.view();
    rig.pointer(PointerAction::Pressed, rig.on_track(0.25, 0.0));
    rig.pointer(PointerAction::Moved, rig.on_track(0.75, 0.0));
    assert_eq!(
        rig.view(),
        view,
        "the view stays put inside the track"
    );
    let expected = view.time_at(0.75);
    assert!(
        (rig.time() - expected).abs() < 0.01,
        "time {} under the pointer {expected}",
        rig.time()
    );
}

#[test]
fn scrubbing_onto_either_edge_stays_on_the_page() {
    let mut rig = Rig::open(Duration::from_secs_f64(13.7), 900.0);
    rig.zoom(6.0, 0.4);
    let view = rig.view();
    rig.pointer(PointerAction::Pressed, rig.on_track(0.5, 0.0));
    for fraction in [1.0, 0.0, 1.0] {
        rig.pointer(
            PointerAction::Moved,
            rig.on_track(fraction, 0.0),
        );
        rig.frames(3);
        assert_eq!(
            rig.view(),
            view,
            "at the edge {fraction}, nothing scrolls"
        );
    }
    rig.pointer(PointerAction::Released, rig.on_track(1.0, 0.0));
    rig.frames(3);
    assert_eq!(rig.view(), view, "nor once released there");
}

#[test]
fn dragging_past_the_edge_scrolls_a_little_each_frame() {
    let mut rig = Rig::open(Duration::from_secs(60), 900.0);
    rig.zoom(20.0, 0.0);
    let length = rig.view().length();
    rig.pointer(PointerAction::Pressed, rig.on_track(0.5, 0.0));
    rig.pointer(PointerAction::Moved, rig.on_track(1.0, 30.0));
    let mut last = rig.view().start();
    for frame in 0..20 {
        rig.frames(1);
        let start = rig.view().start();
        let moved = start - last;
        assert!(moved >= 0.0, "frame {frame} scrolled backwards");
        assert!(
            moved < length * 0.1,
            "frame {frame} jumped {moved} s of a {length} s view"
        );
        // The playhead rides the edge.
        assert!((rig.time() - rig.view().end()).abs() < 0.001);
        last = start;
    }
    assert!(last > 0.0, "it scrolled at all");
    assert!(
        last < 60.0 - length,
        "and didn't reach the end in 20 frames"
    );

    // Back inside, scrolling stops.
    rig.pointer(PointerAction::Moved, rig.on_track(0.5, 0.0));
    let held = rig.view();
    rig.frames(5);
    assert_eq!(rig.view(), held);
}

#[test]
fn dragging_past_the_start_scrolls_back() {
    let mut rig = Rig::open(Duration::from_secs(60), 900.0);
    rig.zoom(20.0, 0.5);
    let from = rig.view().start();
    rig.pointer(PointerAction::Pressed, rig.on_track(0.5, 0.0));
    rig.pointer(PointerAction::Moved, rig.on_track(0.0, -30.0));
    rig.frames(10);
    let start = rig.view().start();
    assert!(
        start < from && start > 0.0,
        "scrolled back from {from} to {start}"
    );
    assert!(
        (rig.time() - start).abs() < 0.001,
        "the playhead rides the start"
    );
}

#[test]
fn playback_turns_the_page() {
    let mut rig = Rig::open(Duration::from_secs(10), 900.0);
    rig.zoom(10.0, 0.0);
    let length = rig.view().length();
    rig.parts.clock.toggle();
    // Play for a little over one view's worth of time.
    let frames = (length * 1.2 / REFRESH.as_secs_f64()).ceil() as u32;
    rig.frames(frames);
    let view = rig.view();
    assert!(view.start() > 0.0, "the view moved on");
    assert!(
        (view.start()..=view.end()).contains(&rig.time()),
        "and shows the playhead"
    );
}

#[test]
fn repeated_presses_shuttle_faster_and_a_pause_resets() {
    let rig = Rig::open(Duration::from_secs(10), 900.0);
    let clock = &rig.parts.clock;
    let state = clock.state;
    // Mid-scene, so there is room to play backwards.
    clock.seek(Duration::from_secs(5));
    clock.play(true);
    assert_eq!(clock.speed.get_untracked(), 1.0);
    clock.play(true);
    clock.play(true);
    assert_eq!(clock.speed.get_untracked(), 4.0);
    clock.play(false);
    assert!(state.get_untracked().reversing, "J plays backwards");
    assert_eq!(
        clock.speed.get_untracked(),
        1.0,
        "a change of direction starts at 1x"
    );
    clock.toggle();
    assert!(!state.get_untracked().playing);
    clock.set_speed(0.5);
    clock.play(true);
    assert_eq!(clock.speed.get_untracked(), 0.5);
    clock.play(true);
    assert_eq!(
        clock.speed.get_untracked(),
        1.0,
        "the shuttle multiplies the chosen speed"
    );
}

#[test]
fn space_plays_and_pauses_even_after_clicking_a_button() {
    let mut rig = Rig::open(Duration::from_secs(10), 900.0);
    let playing =
        |rig: &Rig| rig.parts.clock.state.get_untracked().playing;
    let play =
        PLAY.get().expect("the timeline recorded its play button");

    // Clicking the play button plays, and leaves it focused.
    rig.click(play);
    assert!(playing(&rig), "the click played");
    // Space now toggles once; were the focused button pressed too, the
    // two toggles would cancel out.
    rig.key(NamedKey::Space, KeyCode::Space);
    assert!(!playing(&rig), "Space paused, and only once");
    rig.key(NamedKey::Space, KeyCode::Space);
    assert!(playing(&rig), "and plays again");
}

/// A renderer that accepts every frame and draws nothing.
#[derive(Debug, Default)]
struct Nothing {
    atlas: zgui::atlas::MemorySink,
    target: Option<RenderTarget>,
}

impl Renderer for Nothing {
    fn capabilities(&self) -> zgui::render::RenderCapabilities {
        zgui::render::RenderCapabilities::MINIMAL
    }

    fn configure(&mut self, target: RenderTarget) {
        self.target = Some(target);
    }

    fn target(&self) -> Option<RenderTarget> {
        self.target
    }

    fn register_external(
        &mut self,
        _texture: zgui::render::ExternalTexture,
    ) -> zgui::render::TextureHandle {
        zgui::render::TextureHandle(0)
    }

    fn release_external(
        &mut self,
        _handle: zgui::render::TextureHandle,
    ) {
    }

    fn memory(&self) -> zgui::render::MemoryReport {
        zgui::render::MemoryReport::default()
    }

    fn draw(
        &mut self,
        _scene: &zgui::scene::Scene,
        _damage: &zgui::bits::DamageSet,
    ) -> zgui::render::FrameOutcome {
        zgui::render::FrameOutcome::Presented(
            zgui::render::FrameStats::default(),
        )
    }

    fn texture_sink(&mut self) -> &mut dyn zgui::atlas::TextureSink {
        &mut self.atlas
    }
}
