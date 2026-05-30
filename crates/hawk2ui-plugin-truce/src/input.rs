//! Translating native window events into the editor entry's per-frame input.
//!
//! `hawk2ui-host-baseview` already converts native Baseview events into
//! [`PluginHostEvent`]s and records them into a shared sink; the live render
//! producer drains that sink each frame and calls this to project the
//! author-facing subset into [`FrameInput`]s for `host.events` (Decision 0004).
//! Only pointer / keyboard / focus cross — resize, DPI, lifecycle, and
//! frame-presented events are engine-handled (D7) and dropped here.

use hawk2ui_host::PluginHostEvent;
use hawk2ui_script::FrameInput;

/// Projects the author-facing input events (pointer / keyboard / focus) from a
/// drained batch of [`PluginHostEvent`]s into [`FrameInput`]s, preserving arrival
/// order (Decision 0004 D3) and dropping every engine-handled event (D7).
///
/// The drop is a wildcard, not an enumeration: a `PluginHostEvent` variant added
/// later is engine-handled by default and stays out of `host.events` until
/// deliberately promoted into the author-facing surface.
pub(crate) fn frame_inputs_from_host_events(events: &[PluginHostEvent]) -> Vec<FrameInput> {
    events
        .iter()
        .filter_map(|event| match event {
            PluginHostEvent::PointerRouted(pointer) => Some(FrameInput::Pointer {
                x: pointer.x,
                y: pointer.y,
                button: pointer.button.clone(),
            }),
            PluginHostEvent::KeyboardRouted(key) => Some(FrameInput::Key {
                key: key.key.clone(),
                pressed: key.pressed,
            }),
            PluginHostEvent::FocusRouted(focused) => Some(FrameInput::Focus { focused: *focused }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use hawk2ui_host::{KeyboardInput, PluginHostEvent, PointerInput, SurfaceMetrics};
    use hawk2ui_script::FrameInput;

    use super::frame_inputs_from_host_events;

    #[test]
    fn projects_pointer_keyboard_and_focus_in_arrival_order() {
        let events = vec![
            PluginHostEvent::PointerRouted(PointerInput::new(12.5, 34.0, "left-down")),
            PluginHostEvent::KeyboardRouted(KeyboardInput::new("a", true)),
            PluginHostEvent::FocusRouted(true),
        ];
        assert_eq!(
            frame_inputs_from_host_events(&events),
            vec![
                FrameInput::Pointer {
                    x: 12.5,
                    y: 34.0,
                    button: "left-down".into(),
                },
                FrameInput::Key {
                    key: "a".into(),
                    pressed: true,
                },
                FrameInput::Focus { focused: true },
            ]
        );
    }

    #[test]
    fn drops_engine_handled_events() {
        // Resize / DPI / lifecycle / frame-presented are engine-handled (D7) and
        // never reach the author's host.events — only the focus event survives.
        let metrics = SurfaceMetrics::new(320.0, 180.0, 1.0);
        let events = vec![
            PluginHostEvent::HostResize(metrics),
            PluginHostEvent::DpiChanged(2.0),
            PluginHostEvent::FocusRouted(false),
            PluginHostEvent::FramePresented {
                frame_id: 1,
                metrics,
            },
            PluginHostEvent::SafeTeardownComplete,
        ];
        assert_eq!(
            frame_inputs_from_host_events(&events),
            vec![FrameInput::Focus { focused: false }]
        );
    }
}
