use hawk2ui_host::{
    ClipboardCapability, DesktopHostAdapter, DesktopWindowConfig, HostPlatformHandle,
    KeyboardInput, LinuxWindowSystem, PointerInput, RendererResizeBridge, SurfaceMetrics,
    WindowMode,
};
use hawk2ui_host_winit::{WinitDesktopAdapter, WinitPlatformFixture};

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
