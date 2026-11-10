//! A seekable example clock, independent of MotionGfx timelines.
//!
//! Demos can sample a timeline or calculate a picture directly from
//! [`PlaybackState::time`]. Stateful simulations must implement their own
//! reset/replay if they want to reconstruct an earlier time.

use std::time::Duration;

use vello::Scene;
use vello::kurbo::{
    Affine, BezPath, Circle, Line, Point, Rect, Size, Stroke,
};
use vello::peniko::{Color, Fill};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::labels;

#[derive(Clone, Copy, Debug, Default)]
pub enum PlaybackMode {
    #[default]
    Once,
    Loop,
    PingPong,
    /// Play the introduction once, then repeat the remainder.
    LoopFrom(Duration),
}

#[derive(Clone, Copy, Debug)]
pub struct PlaybackConfig {
    duration: Duration,
    mode: PlaybackMode,
}

impl PlaybackConfig {
    /// Duration is the preview range; it need not come from a timeline.
    pub fn new(duration: Duration) -> Self {
        assert!(
            !duration.is_zero(),
            "preview duration must be positive"
        );
        Self {
            duration,
            mode: PlaybackMode::Once,
        }
    }

    pub fn with_mode(mut self, mode: PlaybackMode) -> Self {
        if let PlaybackMode::LoopFrom(start) = mode {
            assert!(
                start < self.duration,
                "loop start must precede the end"
            );
        }
        self.mode = mode;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PlaybackState {
    pub time: Duration,
    pub duration: Duration,
    pub playing: bool,
    pub reversing: bool,
}

/// No wall clock or renderer: advancing is explicit and testable.
pub struct Playback {
    config: PlaybackConfig,
    phase: f64,
    playing: bool,
}

impl Playback {
    pub fn new(config: PlaybackConfig) -> Self {
        Self {
            config,
            phase: 0.0,
            playing: true,
        }
    }

    pub fn advance(&mut self, delta: Duration) {
        if !self.playing {
            return;
        }
        self.phase += delta.as_secs_f64();
        let end = self.config.duration.as_secs_f64();
        match self.config.mode {
            PlaybackMode::Once => {
                if self.phase >= end {
                    self.phase = end;
                    self.playing = false;
                }
            }
            PlaybackMode::Loop => self.phase %= end,
            PlaybackMode::PingPong => self.phase %= 2.0 * end,
            PlaybackMode::LoopFrom(start) if self.phase >= end => {
                let start = start.as_secs_f64();
                self.phase =
                    start + (self.phase - end) % (end - start);
            }
            PlaybackMode::LoopFrom(_) => {}
        }
    }

    pub fn state(&self) -> PlaybackState {
        let end = self.config.duration.as_secs_f64();
        let reversing =
            matches!(self.config.mode, PlaybackMode::PingPong)
                && self.phase > end;
        let seconds = if reversing {
            2.0 * end - self.phase
        } else {
            self.phase
        };
        PlaybackState {
            time: Duration::from_secs_f64(seconds)
                .min(self.config.duration),
            duration: self.config.duration,
            playing: self.playing,
            reversing,
        }
    }

    /// Seek pauses playback. Resuming after a seek moves forward.
    pub fn seek(&mut self, time: Duration) {
        self.phase = time.min(self.config.duration).as_secs_f64();
        self.pause();
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn toggle(&mut self) {
        if !self.playing
            && self.phase >= self.config.duration.as_secs_f64()
            && matches!(self.config.mode, PlaybackMode::Once)
        {
            self.phase = 0.0;
        }
        self.playing = !self.playing;
    }
}

/// A host-owned control strip, using logical WINDOW coordinates. It is
/// separate from each demo's fitted canvas and its MotionGfx track layout.
pub(crate) struct PlaybackControls {
    pub clock: Playback,
    cursor: Option<Point>,
    dragging: bool,
}

impl PlaybackControls {
    pub const HEIGHT: f64 = 56.0;

    pub fn new(config: PlaybackConfig) -> Self {
        Self {
            clock: Playback::new(config),
            cursor: None,
            dragging: false,
        }
    }

    fn bar(size: Size) -> Line {
        let y = (size.height - 17.0).max(0.0);
        Line::new((60.0, y), ((size.width - 24.0).max(61.0), y))
    }

    fn seek_x(&mut self, x: f64, size: Size) {
        let bar = Self::bar(size);
        let fraction =
            ((x - bar.p0.x) / (bar.p1.x - bar.p0.x)).clamp(0.0, 1.0);
        let end = self.clock.state().duration;
        self.clock.seek(end.mul_f64(fraction));
    }

    pub fn resized(&mut self) {
        self.cursor = None;
        self.dragging = false;
    }

    /// True when the controls consume the input rather than the demo.
    pub fn handle_event(
        &mut self,
        event: &WindowEvent,
        scale: f64,
        size: Size,
    ) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some(Point::new(
                    position.x / scale,
                    position.y / scale,
                ));
                if self.dragging {
                    self.seek_x(position.x / scale, size);
                    return true;
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if *state == ElementState::Released {
                    return std::mem::take(&mut self.dragging);
                }
                if let Some(cursor) = self.cursor {
                    let top = (size.height - Self::HEIGHT).max(0.0);
                    if cursor.y >= top && cursor.y <= size.height {
                        if cursor.x < 48.0 {
                            self.clock.toggle();
                        } else {
                            self.dragging = true;
                            self.seek_x(cursor.x, size);
                        }
                        return true;
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed =>
            {
                let state = self.clock.state();
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Space) => {
                        if !event.repeat && !self.dragging {
                            self.clock.toggle();
                        }
                    }
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        self.clock.seek(state.time.saturating_sub(
                            Duration::from_millis(250),
                        ))
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                        self.clock.seek(state.time.saturating_add(
                            Duration::from_millis(250),
                        ))
                    }
                    PhysicalKey::Code(KeyCode::Home) => {
                        self.clock.seek(Duration::ZERO)
                    }
                    PhysicalKey::Code(KeyCode::End) => {
                        self.clock.seek(state.duration)
                    }
                    _ => return false,
                }
                return true;
            }
            WindowEvent::Focused(false) => {
                self.clock.pause();
                self.resized();
            }
            WindowEvent::CursorLeft { .. } if !self.dragging => {
                self.cursor = None
            }
            _ => {}
        }
        false
    }

    pub fn draw(&self, scene: &mut Scene, scale: f64, size: Size) {
        let xf = Affine::scale(scale);
        let top = (size.height - Self::HEIGHT).max(0.0);
        let ink = Color::from_rgb8(146, 157, 184);
        let accent = Color::from_rgb8(72, 222, 194);
        scene.fill(
            Fill::NonZero,
            xf,
            Color::from_rgb8(17, 21, 34),
            None,
            &Rect::new(0.0, top, size.width, size.height),
        );
        let state = self.clock.state();
        let text = if size.width >= 700.0 {
            format!(
                "{:.2} / {:.2} S  -  DRAG SEEK / SPACE PLAY-PAUSE / ARROWS / HOME-END{}",
                state.time.as_secs_f64(),
                state.duration.as_secs_f64(),
                if state.reversing { " / REVERSE" } else { "" }
            )
        } else {
            format!(
                "{:.2} / {:.2} S",
                state.time.as_secs_f64(),
                state.duration.as_secs_f64()
            )
        };
        labels::draw(
            scene,
            xf,
            Point::new(60.0, top + 8.0),
            &text,
            ink,
            1.3,
        );

        if state.playing {
            for x in [19.0, 28.0] {
                scene.fill(
                    Fill::NonZero,
                    xf,
                    accent,
                    None,
                    &Rect::new(x, top + 18.0, x + 5.0, top + 36.0),
                );
            }
        } else {
            let mut triangle = BezPath::new();
            triangle.move_to((20.0, top + 17.0));
            triangle.line_to((20.0, top + 37.0));
            triangle.line_to((35.0, top + 27.0));
            triangle.close_path();
            scene.fill(Fill::NonZero, xf, accent, None, &triangle);
        }
        let bar = Self::bar(size);
        let progress =
            state.time.as_secs_f64() / state.duration.as_secs_f64();
        let handle = bar.p0 + (bar.p1 - bar.p0) * progress;
        scene.stroke(&Stroke::new(3.0), xf, ink, None, &bar);
        scene.stroke(
            &Stroke::new(3.0),
            xf,
            accent,
            None,
            &Line::new(bar.p0, handle),
        );
        scene.fill(
            Fill::NonZero,
            xf,
            accent,
            None,
            &Circle::new(handle, 6.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seconds(t: f64) -> Duration {
        Duration::from_secs_f64(t)
    }

    #[test]
    fn pause_seek_and_pingpong() {
        let mut clock = Playback::new(
            PlaybackConfig::new(seconds(10.0))
                .with_mode(PlaybackMode::PingPong),
        );
        clock.advance(seconds(5.0));
        assert_eq!(clock.state().time, seconds(5.0));
        clock.seek(seconds(3.0));
        clock.advance(seconds(100.0));
        assert_eq!(clock.state().time, seconds(3.0));
        clock.toggle();
        clock.advance(seconds(8.0));
        assert_eq!(clock.state().time, seconds(9.0));
        assert!(clock.state().reversing);
        clock.seek(seconds(100.0));
        assert_eq!(clock.state().time, seconds(10.0));
        assert!(!clock.state().reversing);
    }

    #[test]
    fn once_stops_and_can_restart() {
        let mut clock =
            Playback::new(PlaybackConfig::new(seconds(2.0)));
        clock.advance(seconds(5.0));
        assert_eq!(clock.state().time, seconds(2.0));
        assert!(!clock.state().playing);
        clock.toggle();
        assert_eq!(clock.state().time, Duration::ZERO);
        assert!(clock.state().playing);
    }

    #[test]
    fn whole_range_loop_wraps() {
        let mut clock = Playback::new(
            PlaybackConfig::new(seconds(10.0))
                .with_mode(PlaybackMode::Loop),
        );
        clock.advance(seconds(25.0));
        assert_eq!(clock.state().time, seconds(5.0));
        assert!(clock.state().playing);
    }

    #[test]
    fn loop_preserves_an_intro_and_handles_large_steps() {
        let mut clock = Playback::new(
            PlaybackConfig::new(seconds(10.0))
                .with_mode(PlaybackMode::LoopFrom(seconds(3.0))),
        );
        clock.advance(seconds(2.0));
        assert_eq!(clock.state().time, seconds(2.0));
        clock.advance(seconds(24.0));
        assert_eq!(clock.state().time, seconds(5.0));
        clock.seek(seconds(1.0));
        assert_eq!(clock.state().time, seconds(1.0));
    }

    #[test]
    fn controls_map_resized_bar_to_time() {
        let mut controls =
            PlaybackControls::new(PlaybackConfig::new(seconds(10.0)));
        for size in
            [Size::new(1120.0, 856.0), Size::new(560.0, 456.0)]
        {
            let bar = PlaybackControls::bar(size);
            controls.seek_x(bar.p0.x, size);
            assert_eq!(controls.clock.state().time, Duration::ZERO);
            controls.seek_x((bar.p0.x + bar.p1.x) / 2.0, size);
            assert_eq!(controls.clock.state().time, seconds(5.0));
            controls.seek_x(bar.p1.x + 100.0, size);
            assert_eq!(controls.clock.state().time, seconds(10.0));
        }
    }

    #[test]
    fn zero_duration_is_rejected() {
        assert!(
            std::panic::catch_unwind(|| PlaybackConfig::new(
                Duration::ZERO
            ))
            .is_err()
        );
    }
}
