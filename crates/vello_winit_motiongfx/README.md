# Vello Winit MotionGfx

Reusable native window/GPU host extracted from `vello_winit_example`.
`Window::new(title, canvas_size).run(animation)` fits the logical canvas into
the window, samples the shared `motiongfx_2d::CompiledScene` and passes its state
to a separately owned `vello_motiongfx::Renderer`. Duration comes from
the track, including waits; the current preview requires a nonzero duration.

The existing play/pause and scrub controls are shared with all examples:
Space toggles playback, the bottom bar supports click/drag seeking, arrows
step by 250ms, and Home/End seek to the endpoints. Seeking pauses playback.

With the `audio` feature, `Window::new(..).audio().run(animation)` plays the
scene's sound events (notes and samples from `motiongfx_audio`) through the
default output device. Sound follows the clock: it plays while time moves
forwards, continues across loop wraps, and stops on a pause, seek, focus
loss or ping-pong reverse; resuming starts only events after the new time.
At the natural end of a single play, the last notes ring out. If no device
can be opened, the preview runs silently and prints why to stderr. The
sound itself (scheduling, voices, the device stream) lives in
`motiongfx_audio`; this crate only connects it to the clock. Other hosts
driving the same `Playback` clock (such as `zgui_editor`) use the public
`audio::AudioSync` for the same behaviour.

`VelloDemo` and `VelloWinitApp` remain available for custom immediate drawing
and procedural demonstrations. Their playback clock is independent of an
animation timeline. No editor or alternate timeline compiler is introduced.
The legacy `labels::draw` helper is re-exported from the Vello crate's
temporary lettering renderer; new authored scenes should use `Text` objects.

Moxie's `crates/moxie/src/playback.rs` was inspected: its controls set an
existing MotionGfx timeline's target time and pause Bevy's `RealtimePlayer`.
Those UI systems depend on Bevy/Fynix, so this host retains its existing
Winit controls and the same target-time sampling approach.
