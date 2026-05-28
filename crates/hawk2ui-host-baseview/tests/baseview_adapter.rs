use hawk2ui_host::{
    HostPlatformHandle, KeyboardInput, PluginEditorConfig, PluginHostAdapter, PluginHostEvent,
    PluginParentHandle, PointerInput, RendererResizeBridge, SurfaceMetrics,
};
use hawk2ui_host_baseview::{
    BaseviewNativeParent, BaseviewNativeParentBackend, BaseviewParentFixture, BaseviewPluginAdapter,
};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_render::Color;
use hawk2ui_runtime::{
    RuntimeSceneBridge, RuntimeSceneFrame, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
    RuntimeVisual,
};
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

#[test]
fn baseview_adapter_attaches_editor_to_daw_owned_parent() {
    let config = PluginEditorConfig::new(
        "editor",
        PluginParentHandle::opaque("daw-parent"),
        SurfaceMetrics::new(640.0, 360.0, 1.0),
    );
    let mut adapter = BaseviewPluginAdapter::attach(config, BaseviewParentFixture::linux_x11())
        .expect("baseview editor attaches");

    assert_eq!(adapter.parent_fixture().id(), "linux-x11-parent");
    assert!(adapter.capabilities().embedded_parent_attachment());
    assert!(!adapter.requested_process_quit());
    assert_eq!(adapter.drain_events().len(), 2);
}

#[test]
fn baseview_parent_fixture_can_wrap_real_host_platform_handle_records() {
    let parent = BaseviewParentFixture::from_platform_handle(
        "host-xcb-parent",
        HostPlatformHandle::linux_xcb(100, 200),
    );
    let config = PluginEditorConfig::new(
        "editor",
        PluginParentHandle::opaque("daw-parent"),
        SurfaceMetrics::new(640.0, 360.0, 1.0),
    );

    let adapter = BaseviewPluginAdapter::attach(config, parent)
        .expect("baseview editor attaches to XCB-compatible parent record");

    assert_eq!(adapter.parent_fixture().id(), "host-xcb-parent");
}

#[test]
fn baseview_native_parent_maps_x11_xcb_xwayland_and_windows_raw_handles() {
    let x11 = BaseviewNativeParent::try_from_handle(HostPlatformHandle::linux_x11(100, 200))
        .expect("x11 parent is supported");
    assert_eq!(x11.handle(), HostPlatformHandle::linux_x11(100, 200));
    assert_eq!(x11.backend(), BaseviewNativeParentBackend::X11);
    assert!(matches!(
        x11.raw_display_handle(),
        RawDisplayHandle::Xlib(display) if display.display as usize == 100
    ));
    assert!(matches!(
        x11.raw_window_handle(),
        RawWindowHandle::Xlib(window) if window.window == 200
    ));

    let xcb = BaseviewNativeParent::try_from_handle(HostPlatformHandle::linux_xcb(300, 400))
        .expect("xcb parent is supported");
    assert_eq!(xcb.backend(), BaseviewNativeParentBackend::Xcb);
    assert!(matches!(
        xcb.raw_display_handle(),
        RawDisplayHandle::Xcb(display) if display.connection as usize == 300
    ));
    assert!(matches!(
        xcb.raw_window_handle(),
        RawWindowHandle::Xcb(window) if window.window == 400
    ));

    let xwayland =
        BaseviewNativeParent::try_from_handle(HostPlatformHandle::linux_xwayland(500, 600))
            .expect("xwayland parent maps through x11 handles");
    assert_eq!(xwayland.backend(), BaseviewNativeParentBackend::XWayland);
    assert!(matches!(
        xwayland.raw_window_handle(),
        RawWindowHandle::Xlib(window) if window.window == 600
    ));

    let windows = BaseviewNativeParent::try_from_handle(HostPlatformHandle::windows_hwnd(700))
        .expect("windows HWND parent is supported");
    assert_eq!(windows.backend(), BaseviewNativeParentBackend::Windows);
    assert!(matches!(
        windows.raw_window_handle(),
        RawWindowHandle::Win32(window) if window.hwnd as usize == 700
    ));

    let macos = BaseviewNativeParent::try_from_handle(HostPlatformHandle::macos_ns_view_in_window(
        800, 900,
    ))
    .expect("macOS AppKit parent is supported when NSWindow and NSView are present");
    assert_eq!(macos.backend(), BaseviewNativeParentBackend::MacOs);
    assert!(matches!(
        macos.raw_window_handle(),
        RawWindowHandle::AppKit(window)
            if window.ns_window as usize == 800 && window.ns_view as usize == 900
    ));
}

#[test]
fn baseview_native_parent_requires_real_nonzero_supported_parent_handles() {
    let invalid_handles = [
        HostPlatformHandle::linux_x11(0, 200),
        HostPlatformHandle::linux_x11(100, 0),
        HostPlatformHandle::linux_xcb(0, 200),
        HostPlatformHandle::linux_xcb(100, 0),
        HostPlatformHandle::linux_xcb(100, u64::from(u32::MAX) + 1),
        HostPlatformHandle::linux_xwayland(0, 200),
        HostPlatformHandle::linux_xwayland(100, 0),
        HostPlatformHandle::windows_hwnd(0),
        HostPlatformHandle::macos_ns_view(0),
        HostPlatformHandle::macos_ns_view(900),
        HostPlatformHandle::macos_ns_view_in_window(0, 900),
        HostPlatformHandle::macos_ns_view_in_window(800, 0),
    ];

    for handle in invalid_handles {
        let error = BaseviewNativeParent::try_from_handle(handle)
            .expect_err("zero native handles must not be passed to baseview");
        assert_eq!(error.rule(), "baseview.native-parent.invalid");
    }

    let error = BaseviewNativeParent::try_from_handle(HostPlatformHandle::linux_wayland(100, 200))
        .expect_err("baseview 0.1 does not support native Wayland parent handles");
    assert_eq!(error.rule(), "baseview.platform.unsupported");
}

#[test]
fn baseview_adapter_exposes_native_parent_and_open_options_for_real_attachment() {
    let adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.5),
        ),
        BaseviewParentFixture::linux_xwayland(),
    )
    .expect("baseview editor attaches");

    let native_parent = adapter
        .native_parent()
        .expect("adapter exposes raw native parent");
    assert_eq!(
        native_parent.backend(),
        BaseviewNativeParentBackend::XWayland
    );
    assert_eq!(adapter.open_options().title, "editor");
    assert_eq!(adapter.open_options().size.width, 320.0);
    assert_eq!(adapter.open_options().size.height, 180.0);
}

#[cfg(target_os = "linux")]
#[test]
fn baseview_open_parented_rejects_cross_target_parent_before_creating_window() {
    let adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ),
        BaseviewParentFixture::from_platform_handle(
            "windows-parent",
            HostPlatformHandle::windows_hwnd(700),
        ),
    )
    .expect("recorded Windows parent is valid but not openable on Linux");

    let result = adapter.open_parented_window(|_| NoopWindowHandler);
    assert!(result.is_err());
    let error = result
        .err()
        .expect("target mismatch must be reported before Baseview tries to open a window");

    assert_eq!(error.rule(), "baseview.native-parent.target-mismatch");
}

#[test]
fn baseview_adapter_rejects_invalid_initial_metrics() {
    let invalid_metrics = [
        SurfaceMetrics::new(0.0, 180.0, 1.0),
        SurfaceMetrics::new(320.0, 0.0, 1.0),
        SurfaceMetrics::new(f64::NAN, 180.0, 1.0),
        SurfaceMetrics::new(320.0, f64::INFINITY, 1.0),
        SurfaceMetrics::new(320.0, 180.0, 0.0),
        SurfaceMetrics::new(320.0, 180.0, f64::INFINITY),
    ];

    for metrics in invalid_metrics {
        let error = BaseviewPluginAdapter::attach(
            PluginEditorConfig::new("editor", PluginParentHandle::opaque("parent"), metrics),
            BaseviewParentFixture::linux_xwayland(),
        )
        .expect_err("invalid metrics must not reach Baseview open options");

        assert_eq!(error.rule(), "baseview.metrics.invalid");
    }
}

#[test]
fn baseview_adapter_ignores_invalid_live_metrics() {
    let initial_metrics = SurfaceMetrics::new(320.0, 180.0, 1.0);
    let mut adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            initial_metrics,
        ),
        BaseviewParentFixture::linux_xwayland(),
    )
    .expect("baseview editor attaches");
    adapter.drain_events();

    adapter.host_resize(SurfaceMetrics::new(0.0, 360.0, 2.0));
    adapter.host_resize(SurfaceMetrics::new(640.0, f64::NAN, 2.0));
    adapter.dpi_changed(0.0);
    adapter.dpi_changed(f64::INFINITY);

    assert_eq!(adapter.metrics(), initial_metrics);
    assert!(adapter.drain_events().is_empty());
}

#[test]
fn baseview_adapter_reports_invalid_live_metrics() {
    let initial_metrics = SurfaceMetrics::new(320.0, 180.0, 1.0);
    let mut adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            initial_metrics,
        ),
        BaseviewParentFixture::linux_xwayland(),
    )
    .expect("baseview editor attaches");
    adapter.drain_events();

    let error = adapter
        .try_host_resize(SurfaceMetrics::new(0.0, 360.0, 2.0))
        .expect_err("invalid host resize metrics must report diagnostics");
    assert_eq!(error.rule(), "baseview.metrics.invalid");

    let error = adapter
        .try_dpi_changed(f64::INFINITY)
        .expect_err("invalid DPI metrics must report diagnostics");
    assert_eq!(error.rule(), "baseview.metrics.invalid");

    assert_eq!(adapter.metrics(), initial_metrics);
    assert!(adapter.drain_events().is_empty());
}

#[test]
fn baseview_adapter_routes_resize_dpi_repaint_focus_keyboard_and_pointer() {
    let mut adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ),
        BaseviewParentFixture::linux_xwayland(),
    )
    .expect("baseview editor attaches");
    let bridge = RendererResizeBridge;

    adapter.host_resize(SurfaceMetrics::new(640.0, 360.0, 1.5));
    adapter.dpi_changed(2.0);
    adapter.schedule_repaint("meter update");
    adapter.route_focus(true);
    adapter.route_keyboard(KeyboardInput::new("Space", true));
    adapter.route_pointer(PointerInput::new(10.0, 12.0, "left"));

    assert_eq!(adapter.metrics().physical_size(), (1280, 720));
    assert_eq!(adapter.repaint_reasons(), ["meter update"]);
    let events = adapter.drain_events();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, PluginHostEvent::HostResize(_)))
    );
    assert!(
        events
            .iter()
            .filter_map(|event| bridge.plugin_event_to_target_request(event, adapter.metrics()))
            .any(|request| request.force_redraw)
    );
}

#[test]
fn baseview_adapter_renders_runtime_scene_into_presented_skia_snapshot() {
    let mut adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ),
        BaseviewParentFixture::linux_xwayland(),
    )
    .expect("baseview editor attaches");
    adapter.drain_events();

    let first_frame = runtime_scene_frame(320.0, 180.0, Color::rgba(12, 34, 56, 255));
    let snapshot = adapter
        .render_scene_frame(&first_frame)
        .expect("runtime frame renders into plugin surface");

    assert_eq!((snapshot.width(), snapshot.height()), (320, 180));
    assert!(snapshot.pixels().iter().any(|pixel| *pixel == 0x0c2238));
    assert_eq!(adapter.presented_frame_count(), 1);
    assert_eq!(
        adapter
            .last_presented_frame()
            .expect("presented snapshot is retained")
            .pixel_at(10, 10),
        Some(0x0c2238)
    );

    adapter
        .try_host_resize(SurfaceMetrics::new(640.0, 360.0, 2.0))
        .expect("host resize updates render target");
    let resized_frame = runtime_scene_frame(640.0, 360.0, Color::rgba(90, 120, 30, 255));
    let resized_snapshot = adapter
        .render_scene_frame(&resized_frame)
        .expect("resized runtime frame renders into plugin surface");

    assert_eq!(
        (resized_snapshot.width(), resized_snapshot.height()),
        (1280, 720)
    );
    assert!(
        resized_snapshot
            .pixels()
            .iter()
            .any(|pixel| *pixel == 0x5a781e)
    );
    assert_eq!(adapter.presented_frame_count(), 2);
}

#[test]
fn baseview_adapter_teardown_destroys_editor_without_process_quit() {
    let mut adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ),
        BaseviewParentFixture::macos_ns_view(),
    )
    .expect("baseview editor attaches");

    adapter.destroy_editor("host closed editor");
    adapter.destroy_editor("duplicate close ignored");

    let events = adapter.drain_events();
    assert!(adapter.destroyed());
    assert!(!adapter.requested_process_quit());
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, PluginHostEvent::EditorDestroyed(_)))
            .count(),
        1
    );
    assert!(events.contains(&PluginHostEvent::SafeTeardownComplete));
}

#[test]
fn baseview_adapter_ignores_host_events_after_destroy() {
    let mut adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ),
        BaseviewParentFixture::linux_xwayland(),
    )
    .expect("baseview editor attaches");
    adapter.drain_events();

    adapter.destroy_editor("host closed editor");
    adapter.drain_events();
    adapter.host_resize(SurfaceMetrics::new(640.0, 360.0, 2.0));
    adapter.dpi_changed(3.0);
    adapter.schedule_repaint("late repaint");
    adapter.route_focus(true);
    adapter.route_keyboard(KeyboardInput::new("Space", true));
    adapter.route_pointer(PointerInput::new(10.0, 12.0, "left"));

    assert_eq!(adapter.metrics(), SurfaceMetrics::new(320.0, 180.0, 1.0));
    assert!(adapter.repaint_reasons().is_empty());
    assert!(adapter.drain_events().is_empty());
}

#[test]
fn baseview_adapter_reports_fallible_host_events_after_destroy() {
    let mut adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ),
        BaseviewParentFixture::linux_xwayland(),
    )
    .expect("baseview editor attaches");
    adapter.drain_events();

    adapter.destroy_editor("host closed editor");
    adapter.drain_events();

    let error = adapter
        .try_host_resize(SurfaceMetrics::new(640.0, 360.0, 2.0))
        .expect_err("fallible resize must report destroyed editor");
    assert_eq!(error.rule(), "baseview.editor.destroyed");

    let error = adapter
        .try_dpi_changed(2.0)
        .expect_err("fallible DPI change must report destroyed editor");
    assert_eq!(error.rule(), "baseview.editor.destroyed");

    assert_eq!(adapter.metrics(), SurfaceMetrics::new(320.0, 180.0, 1.0));
    assert!(adapter.drain_events().is_empty());
}

#[test]
fn baseview_adapter_rejects_native_wayland_parent_handles() {
    let error = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "editor",
            PluginParentHandle::opaque("parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ),
        BaseviewParentFixture::wayland(),
    )
    .expect_err("baseview 0.1 Linux backend cannot attach native Wayland parents");

    assert_eq!(error.rule(), "baseview.platform.unsupported");
}

fn runtime_scene_frame(width: f32, height: f32, color: Color) -> RuntimeSceneFrame {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(width, height)),
        RuntimeVisual::Fill(color),
    ));
    RuntimeSceneBridge::new(Viewport::new(width, height))
        .build(&tree)
        .expect("runtime scene frame builds")
}

struct NoopWindowHandler;

impl baseview::WindowHandler for NoopWindowHandler {
    fn on_frame(&mut self, _window: &mut baseview::Window) {}

    fn on_event(
        &mut self,
        _window: &mut baseview::Window,
        _event: baseview::Event,
    ) -> baseview::EventStatus {
        baseview::EventStatus::Ignored
    }
}
