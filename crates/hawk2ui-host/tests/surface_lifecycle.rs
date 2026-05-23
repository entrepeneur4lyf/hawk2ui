use hawk2ui_host::{
    ClipboardCapability, DesktopHostAdapter, DesktopHostEvent, DesktopWindowConfig, FramePresenter,
    HostCapabilities, HostSurface, KeyboardInput, PointerInput, RecordingDesktopAdapter,
    RecordingFramePresenter, RecordingHostSurface, RepaintRequest, SurfaceEvent, SurfaceMetrics,
    WindowMode,
};

#[test]
fn surface_contract_reports_logical_physical_size_dpi_and_focus() {
    let metrics = SurfaceMetrics::new(640.0, 360.0, 2.0);
    let mut surface = RecordingHostSurface::new(metrics, HostCapabilities::desktop());

    assert_eq!(surface.metrics().logical_width, 640.0);
    assert_eq!(surface.metrics().logical_height, 360.0);
    assert_eq!(surface.metrics().physical_size(), (1280, 720));
    assert_eq!(surface.metrics().scale_factor, 2.0);
    assert!(!surface.has_focus());

    surface.set_focus(true);

    assert!(surface.has_focus());
    assert_eq!(
        surface.drain_events(),
        vec![SurfaceEvent::FocusChanged(true)]
    );
}

#[test]
fn surface_contract_reports_repaint_resize_and_teardown_events() {
    let mut surface = RecordingHostSurface::new(
        SurfaceMetrics::new(400.0, 300.0, 1.0),
        HostCapabilities::desktop(),
    );

    surface.request_repaint(RepaintRequest::full_surface("initial paint"));
    surface.resize(SurfaceMetrics::new(800.0, 600.0, 1.5));
    surface.teardown("window closed");

    assert_eq!(surface.metrics().physical_size(), (1200, 900));
    assert_eq!(
        surface.drain_events(),
        vec![
            SurfaceEvent::RepaintRequested(RepaintRequest::full_surface("initial paint")),
            SurfaceEvent::Resized(SurfaceMetrics::new(800.0, 600.0, 1.5)),
            SurfaceEvent::TeardownRequested("window closed".into()),
        ]
    );
}

#[test]
fn surface_contract_frame_presenter_records_presented_frames() {
    let mut presenter = RecordingFramePresenter::default();

    presenter.present_frame(7, SurfaceMetrics::new(320.0, 240.0, 1.25));

    assert_eq!(presenter.presented_frames()[0].frame_id, 7);
    assert_eq!(
        presenter.presented_frames()[0].metrics.physical_size(),
        (400, 300)
    );
}

#[test]
fn desktop_lifecycle_records_owned_window_creation_and_window_state() {
    let config = DesktopWindowConfig::new("Hawk2UI", SurfaceMetrics::new(1024.0, 768.0, 1.0))
        .with_clipboard(ClipboardCapability::ReadWrite);
    let mut adapter = RecordingDesktopAdapter::create_window(config.clone());

    adapter.request_minimize(true);
    adapter.request_maximize(true);
    adapter.request_fullscreen(true);
    adapter.request_close("user clicked close");

    assert_eq!(adapter.config(), &config);
    assert_eq!(
        adapter.drain_events(),
        vec![
            DesktopHostEvent::WindowCreated(config),
            DesktopHostEvent::ModeChanged(WindowMode::Minimized),
            DesktopHostEvent::ModeChanged(WindowMode::Maximized),
            DesktopHostEvent::ModeChanged(WindowMode::Fullscreen),
            DesktopHostEvent::CloseRequested("user clicked close".into()),
        ]
    );
}

#[test]
fn desktop_lifecycle_records_focus_keyboard_pointer_clipboard_and_dpi() {
    let mut adapter = RecordingDesktopAdapter::create_window(DesktopWindowConfig::new(
        "Hawk2UI",
        SurfaceMetrics::new(800.0, 600.0, 1.0),
    ));
    adapter.drain_events();

    adapter.set_focus(true);
    adapter.keyboard_input(KeyboardInput::new("KeyA", true));
    adapter.pointer_input(PointerInput::new(40.0, 20.0, "primary"));
    adapter.clipboard_available(ClipboardCapability::ReadWrite);
    adapter.dpi_changed(2.0);

    assert_eq!(adapter.metrics().scale_factor, 2.0);
    assert_eq!(
        adapter.drain_events(),
        vec![
            DesktopHostEvent::FocusChanged(true),
            DesktopHostEvent::KeyboardInput(KeyboardInput::new("KeyA", true)),
            DesktopHostEvent::PointerInput(PointerInput::new(40.0, 20.0, "primary")),
            DesktopHostEvent::ClipboardCapabilityChanged(ClipboardCapability::ReadWrite),
            DesktopHostEvent::DpiChanged(2.0),
            DesktopHostEvent::RendererTargetRecreateRequested,
        ]
    );
}
