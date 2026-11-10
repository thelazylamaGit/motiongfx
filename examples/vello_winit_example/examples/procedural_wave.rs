//! No MotionGfx timeline: the same host playback controls drive a pure
//! function of time. Drag the control strip, pause, or seek with arrows.

use std::time::Duration;
use vello::Scene;
use vello::kurbo::{Affine, BezPath, Circle, Size, Stroke};
use vello::peniko::{Color, Fill};
use vello_winit_example::{
    PlaybackConfig, PlaybackMode, PlaybackState, VelloDemo,
    VelloWinitApp,
};
use winit::event_loop::EventLoop;

struct Wave {
    size: Size,
}

impl VelloDemo for Wave {
    fn window_title(&self) -> &'static str {
        "Procedural wave | No timeline"
    }
    fn initial_logical_size(&self) -> (f64, f64) {
        (900.0, 420.0)
    }
    fn size_changed(&mut self, size: Size) {
        self.size = size;
    }
    fn playback_config(&self) -> PlaybackConfig {
        // This is simply the range we want to preview, not a track length.
        PlaybackConfig::new(Duration::from_secs(10))
            .with_mode(PlaybackMode::PingPong)
    }

    fn rebuild_scene(
        &mut self,
        scene: &mut Scene,
        scale: f64,
        playback: PlaybackState,
    ) {
        let time = playback.time.as_secs_f64();
        let wave_y = |u: f64| {
            self.size.height
                * (0.5 + 0.22 * (u * 12.0 - time * 2.0).sin())
        };
        let x = |u: f64| 40.0 + u * (self.size.width - 80.0).max(0.0);
        let xf = Affine::scale(scale);
        let teal = Color::from_rgb8(72, 222, 194);
        let mut path = BezPath::new();
        path.move_to((x(0.0), wave_y(0.0)));
        for i in 1..=240 {
            let u = i as f64 / 240.0;
            path.line_to((x(u), wave_y(u)));
        }
        scene.stroke(&Stroke::new(3.0), xf, teal, None, &path);
        scene.fill(
            Fill::NonZero,
            xf,
            Color::WHITE,
            None,
            &Circle::new((x(0.5), wave_y(0.5)), 9.0),
        );
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloWinitApp::new(Wave {
        size: Size::new(900.0, 420.0),
    });
    event_loop.run_app(&mut app).unwrap();
}
