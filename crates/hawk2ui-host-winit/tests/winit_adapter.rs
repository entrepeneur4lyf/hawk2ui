use hawk2ui_host::{
    ClipboardCapability, DesktopDialogFileFilter, DesktopDialogLevel, DesktopDialogRequest,
    DesktopDialogResponse, DesktopHostAdapter, DesktopHostEvent, DesktopWindowConfig,
    HostPlatformHandle, KeyboardInput, LinuxWindowSystem, PointerInput, RendererResizeBridge,
    SurfaceClipboardRequest, SurfaceMetrics, WindowMode,
};
use hawk2ui_host_winit::{
    ArboardClipboardBackend, WinitClipboardBackend, WinitClipboardBridge, WinitClipboardResponse,
    WinitDesktopAdapter, WinitDialogBackend, WinitDialogBridge, WinitEventTranslator,
    WinitPlatformFixture,
};

#[test]
fn winit_adapter_owns_desktop_window_and_maps_window_controls() {
    let config = DesktopWindowConfig::new("app", SurfaceMetrics::new(800.0, 600.0, 1.0))
        .with_clipboard(ClipboardCapability::ReadWrite);
    let mut adapter = WinitDesktopAdapter::create_window(
        config,
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
    )
    .expect("window fixture creates");

    assert_eq!(adapter.config().title, "app");
    assert_eq!(adapter.mode(), WindowMode::Normal);
    assert!(adapter.capabilities().owns_window());
    assert_eq!(
        adapter.platform_handle().linux_window_system(),
        Some(LinuxWindowSystem::Wayland)
    );

    adapter.request_minimize(true);
    adapter.request_maximize(true);
    adapter.request_fullscreen(true);
    adapter.request_close("user clicked close");

    assert_eq!(adapter.mode(), WindowMode::Fullscreen);
    assert!(adapter.close_requested());
    assert!(adapter
        .drain_events()
        .iter()
        .any(|event| matches!(event, hawk2ui_host::DesktopHostEvent::CloseRequested(reason) if reason == "user clicked close")));
}

#[test]
fn winit_adapter_rejects_invalid_initial_metrics() {
    let invalid_metrics = [
        SurfaceMetrics::new(0.0, 300.0, 1.0),
        SurfaceMetrics::new(400.0, 0.0, 1.0),
        SurfaceMetrics::new(f64::NAN, 300.0, 1.0),
        SurfaceMetrics::new(400.0, f64::INFINITY, 1.0),
        SurfaceMetrics::new(400.0, 300.0, 0.0),
        SurfaceMetrics::new(400.0, 300.0, f64::INFINITY),
    ];

    for metrics in invalid_metrics {
        let error = WinitDesktopAdapter::create_window(
            DesktopWindowConfig::new("app", metrics),
            WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
        )
        .expect_err("invalid metrics must not reach Winit logical size");

        assert_eq!(error.rule(), "desktop.window.invalid-size");
    }
}

#[test]
fn winit_adapter_ignores_invalid_live_resize_and_dpi_metrics() {
    let initial_metrics = SurfaceMetrics::new(400.0, 300.0, 1.0);
    let mut adapter = WinitDesktopAdapter::create_window(
        DesktopWindowConfig::new("app", initial_metrics),
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
    )
    .expect("window fixture creates");
    adapter.drain_events();

    adapter.handle_resize(SurfaceMetrics::new(0.0, 600.0, 2.0));
    adapter.handle_resize(SurfaceMetrics::new(800.0, f64::NAN, 2.0));
    adapter.dpi_changed(0.0);
    adapter.dpi_changed(f64::INFINITY);

    assert_eq!(adapter.metrics(), initial_metrics);
    assert!(adapter.drain_events().is_empty());
}

#[test]
fn winit_adapter_reports_invalid_live_resize_and_dpi_metrics() {
    let initial_metrics = SurfaceMetrics::new(400.0, 300.0, 1.0);
    let mut adapter = WinitDesktopAdapter::create_window(
        DesktopWindowConfig::new("app", initial_metrics),
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
    )
    .expect("window fixture creates");
    adapter.drain_events();

    let error = adapter
        .try_handle_resize(SurfaceMetrics::new(0.0, 600.0, 2.0))
        .expect_err("invalid live resize metrics must report diagnostics");
    assert_eq!(error.rule(), "desktop.window.invalid-size");

    let error = adapter
        .try_dpi_changed(f64::INFINITY)
        .expect_err("invalid live DPI metrics must report diagnostics");
    assert_eq!(error.rule(), "desktop.window.invalid-size");

    assert_eq!(adapter.metrics(), initial_metrics);
    assert!(adapter.drain_events().is_empty());
}

#[test]
fn winit_adapter_close_is_idempotent_and_freezes_late_events() {
    let initial_metrics = SurfaceMetrics::new(400.0, 300.0, 1.0);
    let mut adapter = WinitDesktopAdapter::create_window(
        DesktopWindowConfig::new("app", initial_metrics),
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
    )
    .expect("window fixture creates");
    adapter.drain_events();

    adapter.request_close("first close");
    adapter.request_close("duplicate close");
    adapter.request_maximize(true);
    adapter.handle_resize(SurfaceMetrics::new(800.0, 600.0, 2.0));
    adapter.dpi_changed(2.0);
    adapter.set_focus(true);
    adapter.keyboard_input(KeyboardInput::new("KeyA", true));
    adapter.pointer_input(PointerInput::new(10.0, 20.0, "left"));
    adapter.clipboard_available(ClipboardCapability::ReadWrite);
    adapter.request_repaint("late repaint");

    assert!(adapter.close_requested());
    assert_eq!(adapter.mode(), WindowMode::Normal);
    assert!(!adapter.focused());
    assert_eq!(adapter.metrics(), initial_metrics);
    assert_eq!(adapter.config().clipboard, ClipboardCapability::None);
    assert!(adapter.repaint_requests().is_empty());
    assert_eq!(
        adapter.drain_events(),
        vec![DesktopHostEvent::CloseRequested("first close".into())]
    );
}

#[test]
fn winit_adapter_reports_fallible_events_after_close() {
    let initial_metrics = SurfaceMetrics::new(400.0, 300.0, 1.0);
    let mut adapter = WinitDesktopAdapter::create_window(
        DesktopWindowConfig::new("app", initial_metrics),
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
    )
    .expect("window fixture creates");
    adapter.drain_events();

    adapter.request_close("closed");
    adapter.drain_events();

    let error = adapter
        .try_handle_resize(SurfaceMetrics::new(800.0, 600.0, 2.0))
        .expect_err("fallible resize must report closed window");
    assert_eq!(error.rule(), "desktop.window.closed");

    let error = adapter
        .try_dpi_changed(2.0)
        .expect_err("fallible DPI change must report closed window");
    assert_eq!(error.rule(), "desktop.window.closed");

    assert_eq!(adapter.metrics(), initial_metrics);
    assert!(adapter.drain_events().is_empty());
}

#[test]
fn winit_adapter_routes_focus_keyboard_pointer_clipboard_and_repaint() {
    let mut adapter = WinitDesktopAdapter::create_window(
        DesktopWindowConfig::new("app", SurfaceMetrics::new(400.0, 300.0, 1.0)),
        WinitPlatformFixture::linux(LinuxWindowSystem::X11),
    )
    .expect("window fixture creates");

    adapter.set_focus(true);
    adapter.keyboard_input(KeyboardInput::new("KeyA", true));
    adapter.pointer_input(PointerInput::new(12.0, 24.0, "left"));
    adapter.clipboard_available(ClipboardCapability::ReadWrite);
    adapter.request_repaint("animation tick");

    assert!(adapter.focused());
    assert_eq!(adapter.repaint_requests()[0].reason, "animation tick");
    assert_eq!(adapter.config().clipboard, ClipboardCapability::ReadWrite);
    assert_eq!(adapter.drain_events().len(), 5);
}

#[test]
fn winit_clipboard_bridge_executes_read_write_and_clear_with_capability_checks() {
    let backend = FakeClipboardBackend {
        text: Some("old value".into()),
    };
    let mut bridge = WinitClipboardBridge::new(ClipboardCapability::ReadWrite, backend);

    assert_eq!(
        bridge
            .handle_request(SurfaceClipboardRequest::Read)
            .expect("clipboard read succeeds"),
        WinitClipboardResponse::Text("old value".into())
    );
    assert_eq!(
        bridge
            .handle_request(SurfaceClipboardRequest::Write("new value".into()))
            .expect("clipboard write succeeds"),
        WinitClipboardResponse::Written
    );
    assert_eq!(
        bridge
            .handle_request(SurfaceClipboardRequest::Read)
            .expect("clipboard read after write succeeds"),
        WinitClipboardResponse::Text("new value".into())
    );
    assert_eq!(
        bridge
            .handle_request(SurfaceClipboardRequest::Clear)
            .expect("clipboard clear succeeds"),
        WinitClipboardResponse::Cleared
    );
    assert_eq!(
        bridge
            .handle_request(SurfaceClipboardRequest::Read)
            .expect("clipboard read after clear succeeds"),
        WinitClipboardResponse::Text(String::new())
    );

    let error = WinitClipboardBridge::new(
        ClipboardCapability::Read,
        FakeClipboardBackend {
            text: Some("locked".into()),
        },
    )
    .handle_request(SurfaceClipboardRequest::Write("denied".into()))
    .expect_err("write must require write capability");
    assert_eq!(error.rule(), "desktop.clipboard.write-denied");
}

#[test]
fn winit_adapter_executes_clipboard_requests_through_native_bridge() {
    let mut adapter = WinitDesktopAdapter::create_window(
        DesktopWindowConfig::new("app", SurfaceMetrics::new(400.0, 300.0, 1.0))
            .with_clipboard(ClipboardCapability::ReadWrite),
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
    )
    .expect("window fixture creates");
    adapter.drain_events();
    let mut bridge = WinitClipboardBridge::new(
        adapter.config().clipboard,
        FakeClipboardBackend {
            text: Some("initial".into()),
        },
    );

    assert_eq!(
        adapter
            .try_request_clipboard(
                SurfaceClipboardRequest::Write("from adapter".into()),
                &mut bridge
            )
            .expect("adapter clipboard write succeeds"),
        WinitClipboardResponse::Written
    );
    assert_eq!(
        adapter
            .try_request_clipboard(SurfaceClipboardRequest::Read, &mut bridge)
            .expect("adapter clipboard read succeeds"),
        WinitClipboardResponse::Text("from adapter".into())
    );

    let events = adapter.drain_events();
    assert_eq!(
        events,
        vec![
            DesktopHostEvent::ClipboardRequested(SurfaceClipboardRequest::Write(
                "from adapter".into()
            )),
            DesktopHostEvent::ClipboardRequested(SurfaceClipboardRequest::Read),
        ]
    );
}

#[test]
#[ignore = "requires a native desktop clipboard service"]
fn winit_native_clipboard_backend_smoke_when_enabled() {
    if std::env::var("HAWK2UI_NATIVE_CLIPBOARD_SMOKE").as_deref() != Ok("1") {
        return;
    }

    let mut bridge = WinitClipboardBridge::new(
        ClipboardCapability::ReadWrite,
        ArboardClipboardBackend::new()
            .expect("native clipboard must open when smoke test is enabled"),
    );
    let original = bridge.handle_request(SurfaceClipboardRequest::Read).ok();
    let token = format!("hawk2ui-native-clipboard-smoke-{}", std::process::id());

    bridge
        .handle_request(SurfaceClipboardRequest::Write(token.clone()))
        .expect("native clipboard write succeeds");
    assert_eq!(
        bridge
            .handle_request(SurfaceClipboardRequest::Read)
            .expect("native clipboard read succeeds"),
        WinitClipboardResponse::Text(token)
    );

    match original {
        Some(WinitClipboardResponse::Text(text)) => {
            bridge
                .handle_request(SurfaceClipboardRequest::Write(text))
                .expect("native clipboard original text restores");
        }
        _ => {
            bridge
                .handle_request(SurfaceClipboardRequest::Clear)
                .expect("native clipboard clears after smoke");
        }
    }
}

#[test]
fn winit_dialog_bridge_executes_message_open_and_save_requests() {
    let mut bridge = WinitDialogBridge::new(FakeDialogBackend {
        next_open_file: Some("/tmp/preset.hawk".into()),
        next_save_file: Some("/tmp/output.hawk".into()),
        messages: Vec::new(),
    });

    assert_eq!(
        bridge
            .handle_request(DesktopDialogRequest::Message {
                title: "Confirm".into(),
                message: "Render complete".into(),
                level: DesktopDialogLevel::Info,
            })
            .expect("message dialog succeeds"),
        DesktopDialogResponse::Acknowledged
    );
    assert_eq!(
        bridge
            .handle_request(DesktopDialogRequest::OpenFile {
                title: "Open preset".into(),
                directory: Some("/tmp".into()),
                filters: vec![DesktopDialogFileFilter::new("Hawk", ["hawk"])],
            })
            .expect("open dialog succeeds"),
        DesktopDialogResponse::SelectedFile("/tmp/preset.hawk".into())
    );
    assert_eq!(
        bridge
            .handle_request(DesktopDialogRequest::SaveFile {
                title: "Save preset".into(),
                directory: Some("/tmp".into()),
                file_name: Some("output.hawk".into()),
                filters: vec![DesktopDialogFileFilter::new("Hawk", ["hawk"])],
            })
            .expect("save dialog succeeds"),
        DesktopDialogResponse::SavedFile("/tmp/output.hawk".into())
    );
}

#[test]
fn winit_dialog_bridge_maps_native_cancellation() {
    let mut bridge = WinitDialogBridge::new(FakeDialogBackend {
        next_open_file: None,
        next_save_file: None,
        messages: Vec::new(),
    });

    assert_eq!(
        bridge
            .handle_request(DesktopDialogRequest::OpenFile {
                title: "Open preset".into(),
                directory: None,
                filters: Vec::new(),
            })
            .expect("cancelled open dialog is a valid response"),
        DesktopDialogResponse::Cancelled
    );
    assert_eq!(
        bridge
            .handle_request(DesktopDialogRequest::SaveFile {
                title: "Save preset".into(),
                directory: None,
                file_name: None,
                filters: Vec::new(),
            })
            .expect("cancelled save dialog is a valid response"),
        DesktopDialogResponse::Cancelled
    );
}

#[test]
fn winit_adapter_executes_dialog_requests_through_native_bridge() {
    let mut adapter = WinitDesktopAdapter::create_window(
        DesktopWindowConfig::new("app", SurfaceMetrics::new(400.0, 300.0, 1.0)),
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
    )
    .expect("window fixture creates");
    adapter.drain_events();
    let request = DesktopDialogRequest::Message {
        title: "Ready".into(),
        message: "Project loaded".into(),
        level: DesktopDialogLevel::Info,
    };
    let mut bridge = WinitDialogBridge::new(FakeDialogBackend {
        next_open_file: None,
        next_save_file: None,
        messages: Vec::new(),
    });

    assert_eq!(
        adapter
            .try_request_dialog(request.clone(), &mut bridge)
            .expect("adapter dialog request succeeds"),
        DesktopDialogResponse::Acknowledged
    );

    assert_eq!(
        adapter.drain_events(),
        vec![DesktopHostEvent::DialogRequested(request)]
    );
}

#[derive(Clone, Debug)]
struct FakeClipboardBackend {
    text: Option<String>,
}

impl WinitClipboardBackend for FakeClipboardBackend {
    fn read_text(&mut self) -> Result<String, hawk2ui_host_winit::WinitHostError> {
        Ok(self.text.clone().unwrap_or_default())
    }

    fn write_text(&mut self, text: String) -> Result<(), hawk2ui_host_winit::WinitHostError> {
        self.text = Some(text);
        Ok(())
    }

    fn clear_text(&mut self) -> Result<(), hawk2ui_host_winit::WinitHostError> {
        self.text = Some(String::new());
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct FakeDialogBackend {
    next_open_file: Option<std::path::PathBuf>,
    next_save_file: Option<std::path::PathBuf>,
    messages: Vec<(String, String, DesktopDialogLevel)>,
}

impl WinitDialogBackend for FakeDialogBackend {
    fn show_message(
        &mut self,
        title: String,
        message: String,
        level: DesktopDialogLevel,
    ) -> Result<(), hawk2ui_host_winit::WinitHostError> {
        self.messages.push((title, message, level));
        Ok(())
    }

    fn open_file(
        &mut self,
        _title: String,
        _directory: Option<std::path::PathBuf>,
        _filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<std::path::PathBuf>, hawk2ui_host_winit::WinitHostError> {
        Ok(self.next_open_file.clone())
    }

    fn save_file(
        &mut self,
        _title: String,
        _directory: Option<std::path::PathBuf>,
        _file_name: Option<String>,
        _filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<std::path::PathBuf>, hawk2ui_host_winit::WinitHostError> {
        Ok(self.next_save_file.clone())
    }
}

#[test]
fn winit_event_translator_routes_native_window_ime_drag_pointer_and_close_events() {
    use std::path::PathBuf;
    use winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        event::{DeviceId, ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    };

    let mut translator = WinitEventTranslator::new(SurfaceMetrics::new(800.0, 600.0, 2.0));

    let resized = translator.translate(&WindowEvent::Resized(PhysicalSize::new(1600, 900)));
    assert!(resized.requires_redraw);
    assert!(
        resized
            .events
            .contains(&DesktopHostEvent::Resized(SurfaceMetrics::new(
                800.0, 450.0, 2.0
            )))
    );
    assert!(
        resized
            .events
            .contains(&DesktopHostEvent::RendererTargetRecreateRequested)
    );

    let focused = translator.translate(&WindowEvent::Focused(true));
    assert_eq!(focused.events, [DesktopHostEvent::FocusChanged(true)]);

    let ime = translator.translate(&WindowEvent::Ime(Ime::Preedit("é".into(), Some((0, 2)))));
    assert_eq!(
        ime.events,
        [DesktopHostEvent::ImeInput("preedit:é:0..2".into())]
    );

    let cursor = translator.translate(&WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: PhysicalPosition::new(24.0, 48.0),
    });
    assert_eq!(
        cursor.events,
        [DesktopHostEvent::PointerInput(PointerInput::new(
            12.0, 24.0, "move"
        ))]
    );

    let mouse = translator.translate(&WindowEvent::MouseInput {
        device_id: DeviceId::dummy(),
        state: ElementState::Pressed,
        button: MouseButton::Left,
    });
    assert_eq!(
        mouse.events,
        [DesktopHostEvent::PointerInput(PointerInput::new(
            12.0,
            24.0,
            "left-down"
        ))]
    );

    let wheel = translator.translate(&WindowEvent::MouseWheel {
        device_id: DeviceId::dummy(),
        delta: MouseScrollDelta::LineDelta(1.0, -2.0),
        phase: winit::event::TouchPhase::Moved,
    });
    assert_eq!(
        wheel.events,
        [DesktopHostEvent::PointerInput(PointerInput::new(
            12.0,
            24.0,
            "wheel-lines:1:-2"
        ))]
    );

    let dropped = translator.translate(&WindowEvent::DroppedFile(PathBuf::from("/tmp/preset.h2p")));
    assert_eq!(
        dropped.events,
        [DesktopHostEvent::FileDragDrop(
            "dropped:/tmp/preset.h2p".into()
        )]
    );

    let occluded = translator.translate(&WindowEvent::Occluded(true));
    assert_eq!(
        occluded.events,
        [DesktopHostEvent::WindowOcclusionChanged(true)]
    );

    let close = translator.translate(&WindowEvent::CloseRequested);
    assert!(close.requests_close);
    assert_eq!(
        close.events,
        [DesktopHostEvent::CloseRequested(
            "native close requested".into()
        )]
    );
}

#[test]
fn winit_adapter_recreates_renderer_target_on_resize_maximize_and_dpi() {
    let mut adapter = WinitDesktopAdapter::create_window(
        DesktopWindowConfig::new("app", SurfaceMetrics::new(400.0, 300.0, 1.0)),
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
    )
    .expect("window fixture creates");
    let bridge = RendererResizeBridge;

    adapter.handle_resize(SurfaceMetrics::new(1200.0, 800.0, 1.5));
    adapter.request_maximize(true);
    adapter.dpi_changed(2.0);

    assert_eq!(adapter.metrics().physical_size(), (2400, 1600));
    let requests: Vec<_> = adapter
        .drain_events()
        .iter()
        .filter_map(|event| bridge.desktop_event_to_target_request(event, adapter.metrics()))
        .collect();

    assert!(requests.len() >= 2);
    assert!(requests.iter().all(|request| request.force_redraw));
}

#[test]
fn winit_platform_fixtures_cover_linux_windows_and_macos_handles() {
    let fixtures = [
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
        WinitPlatformFixture::linux(LinuxWindowSystem::X11),
        WinitPlatformFixture::linux(LinuxWindowSystem::Xcb),
        WinitPlatformFixture::windows(),
        WinitPlatformFixture::macos(),
    ];
    let handles: Vec<HostPlatformHandle> =
        fixtures.iter().map(WinitPlatformFixture::handle).collect();

    assert!(handles.contains(&HostPlatformHandle::linux_wayland(1, 2)));
    assert!(handles.contains(&HostPlatformHandle::linux_x11(1, 3)));
    assert!(handles.contains(&HostPlatformHandle::linux_xcb(4, 5)));
    assert!(handles.contains(&HostPlatformHandle::windows_hwnd(6)));
    assert!(handles.contains(&HostPlatformHandle::macos_ns_window(7)));
}
