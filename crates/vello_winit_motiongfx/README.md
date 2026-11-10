# Vello Winit MotionGfx

Reusable native window/GPU host extracted from `vello_winit_example`.
`Window::new(title, canvas_size).run(animation)` fits the logical canvas into
the window, samples the compiled timeline and draws it. Duration comes from
the track, including waits; the current preview requires a nonzero duration.

The existing play/pause and scrub controls are shared with all examples:
Space toggles playback, the bottom bar supports click/drag seeking, arrows
step by 250ms, and Home/End seek to the endpoints. Seeking pauses playback.

`VelloDemo` and `VelloWinitApp` remain available for custom immediate drawing
and procedural demonstrations. Their playback clock is independent of an
animation timeline. No editor or alternate timeline compiler is introduced.
The legacy `labels::draw` helper is re-exported from the Vello crate's
temporary lettering renderer; new authored scenes should use `Text` objects.

Moxie's `crates/moxie/src/playback.rs` was inspected: its controls set an
existing MotionGfx timeline's target time and pause Bevy's `RealtimePlayer`.
Those UI systems depend on Bevy/Fynix, so this host retains its existing
Winit controls and the same target-time sampling approach.
