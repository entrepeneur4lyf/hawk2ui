#![allow(clippy::float_cmp)]

use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use hawk2ui_host::{
    ClipboardCapability, DesktopHostAdapter, DesktopHostEvent, DesktopWindowConfig, FramePresenter,
    HostCapabilities, HostCapability, HostPlatformHandle, HostSurface, KeyboardInput,
    PluginEditorConfig, PluginHostAdapter, PluginHostEvent, PluginParentHandle, PointerInput,
    RepaintRequest, SurfaceClipboardRequest, SurfaceEvent, SurfaceMetrics, SurfaceOwnership,
    SurfaceWindowCommand, SurfaceWindowMode, WindowMode,
    testkit::{
        RecordingDesktopAdapter, RecordingFramePresenter, RecordingHostSurface,
        RecordingPluginAdapter,
    },
};

#[test]
fn platform_handle_diagnostic_converts_to_shared_diagnostic() {
    let error = HostPlatformHandle::macos_ns_view(7)
        .validate_for(SurfaceOwnership::DesktopWindow)
        .expect_err("ownership mismatch is rejected");
    let diagnostic = Diagnostic::from(error);

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic.rule.as_str(),
        "platform.handle-ownership-mismatch"
    );
    assert!(diagnostic.message.contains("NSView"));
}

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
fn surface_metrics_physical_size_guards_non_finite_and_overflow() {
    // Non-finite scale collapses to zero rather than panicking or producing NaN.
    assert_eq!(
        SurfaceMetrics::new(640.0, 360.0, f64::NAN).physical_size(),
        (0, 0)
    );
    assert_eq!(
        SurfaceMetrics::new(640.0, 360.0, f64::INFINITY).physical_size(),
        (0, 0)
    );
    // Negative logical dimensions clamp to zero instead of wrapping under `as u32`.
    assert_eq!(
        SurfaceMetrics::new(-100.0, -50.0, 2.0).physical_size(),
        (0, 0)
    );
    // Dimensions beyond `u32::MAX` saturate rather than wrapping.
    assert_eq!(
        SurfaceMetrics::new(f64::MAX, f64::MAX, 1.0).physical_size(),
        (u32::MAX, u32::MAX)
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
    surface.teardown("window closed".to_owned());

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
fn surface_contract_records_window_clipboard_and_presentation_events() {
    let mut surface = RecordingHostSurface::new(
        SurfaceMetrics::new(400.0, 300.0, 1.0),
        HostCapabilities::desktop(),
    );

    surface.request_window_command(SurfaceWindowCommand::SetMode(SurfaceWindowMode::Maximized));
    surface.request_clipboard(SurfaceClipboardRequest::Write("hello".into()));
    surface.record_presented_frame(42);

    assert_eq!(
        surface.drain_events(),
        vec![
            SurfaceEvent::WindowCommandRequested(SurfaceWindowCommand::SetMode(
                SurfaceWindowMode::Maximized,
            )),
            SurfaceEvent::ClipboardRequested(SurfaceClipboardRequest::Write("hello".into())),
            SurfaceEvent::FramePresented {
                frame_id: 42,
                metrics: SurfaceMetrics::new(400.0, 300.0, 1.0),
            },
        ]
    );
}

#[test]
fn desktop_and_plugin_adapters_implement_common_surface_contract() {
    let mut desktop = RecordingDesktopAdapter::create_window(DesktopWindowConfig::new(
        "Hawk2UI",
        SurfaceMetrics::new(1024.0, 768.0, 1.0),
    ));
    desktop.drain_events();

    desktop.request_repaint(RepaintRequest::full_surface("explicit repaint"));
    desktop.resize(SurfaceMetrics::new(1440.0, 900.0, 2.0));
    desktop.request_window_command(SurfaceWindowCommand::SetMode(SurfaceWindowMode::Fullscreen));
    desktop.request_clipboard(SurfaceClipboardRequest::Read);
    desktop.record_presented_frame(7);

    assert!(desktop.capabilities().supports(HostCapability::OwnsWindow));
    assert_eq!(desktop.metrics().physical_size(), (2880, 1800));
    assert_eq!(
        desktop.drain_events(),
        vec![
            DesktopHostEvent::RepaintRequested(RepaintRequest::full_surface("explicit repaint",)),
            DesktopHostEvent::Resized(SurfaceMetrics::new(1440.0, 900.0, 2.0)),
            DesktopHostEvent::RendererTargetRecreateRequested,
            DesktopHostEvent::ModeChanged(WindowMode::Fullscreen),
            DesktopHostEvent::ClipboardRequested(SurfaceClipboardRequest::Read),
            DesktopHostEvent::FramePresented {
                frame_id: 7,
                metrics: SurfaceMetrics::new(1440.0, 900.0, 2.0),
            },
        ]
    );

    let mut plugin = RecordingPluginAdapter::attach(PluginEditorConfig::new(
        "editor",
        PluginParentHandle::opaque("vst3-parent"),
        SurfaceMetrics::new(640.0, 360.0, 1.0),
    ));
    plugin.drain_events();

    plugin.resize(SurfaceMetrics::new(800.0, 500.0, 1.5));
    plugin.request_repaint(RepaintRequest::full_surface("meter update"));
    plugin.request_window_command(SurfaceWindowCommand::Close("host closed editor".into()));
    plugin.record_presented_frame(3);

    assert!(!plugin.capabilities().supports(HostCapability::OwnsWindow));
    assert_eq!(
        plugin.drain_events(),
        vec![
            PluginHostEvent::HostResize(SurfaceMetrics::new(800.0, 500.0, 1.5)),
            PluginHostEvent::RepaintScheduled("meter update".into()),
            PluginHostEvent::EditorDestroyed("host closed editor".into()),
            PluginHostEvent::SafeTeardownComplete,
        ]
    );
}

#[test]
fn host_surface_is_object_safe_and_routes_common_surface_commands() {
    let mut desktop = RecordingDesktopAdapter::create_window(DesktopWindowConfig::new(
        "Hawk2UI",
        SurfaceMetrics::new(1024.0, 768.0, 1.0),
    ));
    desktop.drain_events();

    let surface: &mut dyn HostSurface = &mut desktop;
    surface.request_repaint(RepaintRequest::full_surface("dyn repaint"));
    surface.resize(SurfaceMetrics::new(1280.0, 720.0, 2.0));
    surface.request_window_command(SurfaceWindowCommand::SetMode(SurfaceWindowMode::Maximized));
    surface.record_presented_frame(99);
    surface.teardown("dyn close".to_owned());

    assert_eq!(
        desktop.drain_events(),
        vec![
            DesktopHostEvent::RepaintRequested(RepaintRequest::full_surface("dyn repaint")),
            DesktopHostEvent::Resized(SurfaceMetrics::new(1280.0, 720.0, 2.0)),
            DesktopHostEvent::RendererTargetRecreateRequested,
            DesktopHostEvent::ModeChanged(WindowMode::Maximized),
            DesktopHostEvent::FramePresented {
                frame_id: 99,
                metrics: SurfaceMetrics::new(1280.0, 720.0, 2.0),
            },
            DesktopHostEvent::CloseRequested("dyn close".into()),
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

#[test]
fn plugin_lifecycle_records_attachment_resize_dpi_repaint_and_input_routing() {
    let mut adapter = RecordingPluginAdapter::attach(PluginEditorConfig::new(
        "editor",
        PluginParentHandle::opaque("vst3-parent"),
        SurfaceMetrics::new(640.0, 360.0, 1.0),
    ));

    adapter.host_resize(SurfaceMetrics::new(800.0, 500.0, 1.0));
    adapter.dpi_changed(1.5);
    adapter.schedule_repaint("parameter changed");
    adapter.route_focus(true);
    adapter.route_keyboard(KeyboardInput::new("Space", true));
    adapter.route_pointer(PointerInput::new(8.0, 16.0, "primary"));

    assert_eq!(adapter.metrics().physical_size(), (1200, 750));
    assert_eq!(
        adapter.drain_events(),
        vec![
            PluginHostEvent::ParentAttached(PluginParentHandle::opaque("vst3-parent")),
            PluginHostEvent::EditorCreated("editor".into()),
            PluginHostEvent::HostResize(SurfaceMetrics::new(800.0, 500.0, 1.0)),
            PluginHostEvent::DpiChanged(1.5),
            PluginHostEvent::RepaintScheduled("parameter changed".into()),
            PluginHostEvent::FocusRouted(true),
            PluginHostEvent::KeyboardRouted(KeyboardInput::new("Space", true)),
            PluginHostEvent::PointerRouted(PointerInput::new(8.0, 16.0, "primary")),
        ]
    );
}

#[test]
fn plugin_lifecycle_records_host_driven_show_hide() {
    let mut adapter = RecordingPluginAdapter::attach(PluginEditorConfig::new(
        "editor",
        PluginParentHandle::opaque("clap-parent"),
        SurfaceMetrics::new(320.0, 240.0, 1.0),
    ));
    adapter.drain_events();

    assert!(adapter.visible());
    adapter.hide_editor("host hid editor");
    adapter.hide_editor("duplicate hide ignored");
    adapter.show_editor("host showed editor");
    adapter.show_editor("duplicate show ignored");

    assert!(adapter.visible());
    assert_eq!(
        adapter.drain_events(),
        vec![
            PluginHostEvent::EditorHidden("host hid editor".into()),
            PluginHostEvent::FocusRouted(false),
            PluginHostEvent::EditorShown("host showed editor".into()),
        ]
    );
}

#[test]
fn plugin_lifecycle_teardown_never_requests_process_quit() {
    let mut adapter = RecordingPluginAdapter::attach(PluginEditorConfig::new(
        "editor",
        PluginParentHandle::opaque("clap-parent"),
        SurfaceMetrics::new(320.0, 240.0, 1.0),
    ));
    adapter.drain_events();

    adapter.destroy_editor("host destroyed editor");

    // A plugin editor structurally cannot request host-process quit: `RequestQuit`
    // is absent from the plugin capability set, and no `PluginHostEvent` or
    // `SurfaceWindowCommand` encodes a quit (`Close` destroys the editor, not the
    // host). This asserts the real enforcement; it fails if `RequestQuit` is ever
    // added to `HostCapabilities::plugin()`.
    assert!(!HostCapabilities::plugin().supports(HostCapability::RequestQuit));
    assert_eq!(
        adapter.drain_events(),
        vec![
            PluginHostEvent::EditorDestroyed("host destroyed editor".into()),
            PluginHostEvent::SafeTeardownComplete,
        ]
    );
}

#[test]
fn plugin_lifecycle_ignores_host_events_after_teardown() {
    let mut adapter = RecordingPluginAdapter::attach(PluginEditorConfig::new(
        "editor",
        PluginParentHandle::opaque("clap-parent"),
        SurfaceMetrics::new(320.0, 240.0, 1.0),
    ));
    adapter.drain_events();

    adapter.destroy_editor("host destroyed editor");
    adapter.drain_events();
    adapter.host_resize(SurfaceMetrics::new(640.0, 480.0, 2.0));
    adapter.dpi_changed(3.0);
    adapter.schedule_repaint("late repaint");
    adapter.route_focus(true);
    adapter.route_keyboard(KeyboardInput::new("Space", true));
    adapter.route_pointer(PointerInput::new(8.0, 16.0, "primary"));

    assert_eq!(adapter.metrics(), SurfaceMetrics::new(320.0, 240.0, 1.0));
    assert!(adapter.drain_events().is_empty());
}

#[test]
fn platform_handles_support_windows_macos_wayland_x11_xcb_and_xwayland() {
    use hawk2ui_host::{HostPlatformHandle, LinuxWindowSystem, SurfaceOwnership};

    let handles = [
        HostPlatformHandle::windows_hwnd(1),
        HostPlatformHandle::macos_ns_view(2),
        HostPlatformHandle::macos_ns_view_in_window(3, 4),
        HostPlatformHandle::macos_ns_window(5),
        HostPlatformHandle::linux_wayland(6, 7),
        HostPlatformHandle::linux_x11(8, 9),
        HostPlatformHandle::linux_xcb(10, 11),
        HostPlatformHandle::linux_xwayland(12, 13),
    ];

    assert_eq!(
        handles[4].linux_window_system(),
        Some(LinuxWindowSystem::Wayland)
    );
    assert_eq!(
        handles[5].linux_window_system(),
        Some(LinuxWindowSystem::X11)
    );
    assert_eq!(
        handles[6].linux_window_system(),
        Some(LinuxWindowSystem::Xcb)
    );
    assert_eq!(
        handles[7].linux_window_system(),
        Some(LinuxWindowSystem::XWayland)
    );
    assert!(
        handles[0]
            .validate_for(SurfaceOwnership::DesktopWindow)
            .is_ok()
    );
}

#[test]
fn platform_handles_diagnose_unsupported_surface_combinations() {
    use hawk2ui_host::{HostPlatformHandle, SurfaceOwnership};

    let plugin_window_error = HostPlatformHandle::macos_ns_window(3)
        .validate_for(SurfaceOwnership::PluginEditor)
        .expect_err("plugin editors must attach to child views, not top-level windows");
    assert_eq!(
        plugin_window_error.code,
        "platform.handle-ownership-mismatch"
    );

    let desktop_view_error = HostPlatformHandle::macos_ns_view(2)
        .validate_for(SurfaceOwnership::DesktopWindow)
        .expect_err("desktop windows need top-level window handles");
    assert_eq!(
        desktop_view_error.code,
        "platform.handle-ownership-mismatch"
    );
}

#[test]
fn renderer_resize_bridge_recreates_target_and_forces_redraw_on_maximize() {
    use hawk2ui_host::{RendererResizeBridge, RendererTargetRequest};

    let bridge = RendererResizeBridge;
    let request = bridge
        .desktop_event_to_target_request(
            &DesktopHostEvent::ModeChanged(WindowMode::Maximized),
            SurfaceMetrics::new(1440.0, 900.0, 2.0),
        )
        .expect("maximize should recreate renderer target");

    assert_eq!(
        request,
        RendererTargetRequest::recreate(
            SurfaceMetrics::new(1440.0, 900.0, 2.0),
            "desktop window mode changed: Maximized",
        )
    );
    assert!(request.force_redraw);
}

#[test]
fn renderer_resize_bridge_recreates_target_and_forces_redraw_on_dpi_change() {
    use hawk2ui_host::{HostSurfaceUpdateRequest, RendererResizeBridge, RendererTargetRequest};

    let bridge = RendererResizeBridge;
    let metrics = SurfaceMetrics::new(800.0, 600.0, 1.75);
    let request = bridge
        .desktop_event_to_target_request(&DesktopHostEvent::DpiChanged(1.75), metrics)
        .expect("DPI changes should recreate renderer target");

    assert_eq!(
        request,
        RendererTargetRequest::recreate(
            SurfaceMetrics::new(800.0, 600.0, 1.75),
            "desktop DPI changed to 1.75",
        )
    );
    assert_eq!(request.metrics.physical_size(), (1400, 1050));

    let update = bridge
        .desktop_event_to_update_request(&DesktopHostEvent::DpiChanged(1.75), metrics)
        .expect("DPI changes should invalidate layout and renderer targets");
    assert_eq!(
        update,
        HostSurfaceUpdateRequest::new(
            metrics,
            RendererTargetRequest::recreate(metrics, "desktop DPI changed to 1.75"),
            "desktop DPI changed to 1.75",
        )
    );
    assert!(update.invalidate_layout);
    assert_eq!(update.logical_viewport(), (800.0, 600.0));
    assert_eq!(update.physical_size(), (1400, 1050));
}

#[test]
fn renderer_resize_bridge_invalidates_layout_for_surface_desktop_and_plugin_size_changes() {
    use hawk2ui_host::{HostSurfaceUpdateRequest, RendererResizeBridge, RendererTargetRequest};

    let bridge = RendererResizeBridge;
    let resized = SurfaceMetrics::new(1024.0, 768.0, 1.5);
    let surface_update = bridge
        .surface_event_to_update_request(&SurfaceEvent::Resized(resized))
        .expect("surface resize should produce update request");
    assert_eq!(
        surface_update,
        HostSurfaceUpdateRequest::new(
            resized,
            RendererTargetRequest::recreate(resized, "surface resized"),
            "surface resized",
        )
    );

    let plugin_update = bridge
        .plugin_event_to_update_request(&PluginHostEvent::HostResize(resized), resized)
        .expect("plugin host resize should produce update request");
    assert_eq!(plugin_update.logical_viewport(), (1024.0, 768.0));
    assert_eq!(plugin_update.physical_size(), (1536, 1152));
    assert!(plugin_update.invalidate_layout);
    assert!(plugin_update.renderer_target.force_redraw);
}
