use hawk2ui_host::{
    HostPlatformHandle, KeyboardInput, PluginEditorConfig, PluginHostAdapter, PluginHostEvent,
    PluginParentHandle, PointerInput, RendererResizeBridge, SurfaceMetrics,
};
use hawk2ui_host_baseview::{BaseviewParentFixture, BaseviewPluginAdapter};

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
