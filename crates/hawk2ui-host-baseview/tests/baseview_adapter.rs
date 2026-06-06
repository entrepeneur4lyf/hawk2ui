use hawk2ui_build::{
    ArtifactHash, ArtifactSchemaVersion, ArtifactSignatureVerifier, ArtifactSigningKey,
    CompiledScriptRecord, HawkManifest, SealedArtifact,
};
use hawk2ui_host::{
    HostPlatformHandle, KeyboardInput, PluginEditorConfig, PluginHostAdapter, PluginHostEvent,
    PluginParentHandle, PointerInput, RendererResizeBridge, SurfaceMetrics,
};
use hawk2ui_host_baseview::{
    BaseviewCapabilities, BaseviewClapRuntimeEditor, BaseviewClapRuntimeEditorHost,
    BaseviewClapRuntimeEditorHostAbiBridge, BaseviewClapRuntimeEditorHostCommand,
    BaseviewClapRuntimeEditorHostResponse, BaseviewEventTranslator, BaseviewNativeParent,
    BaseviewNativeParentBackend, BaseviewParentFixture, BaseviewPluginAdapter,
};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_plugin::{
    BundleOutput, FormatMetadata, FrameDropPolicy, ParameterModel, ParameterValue, PluginEditor,
    PluginEditorSize, PluginStateEnvelope, RealtimeVisualFrameGate, RealtimeVisualPacket,
    RealtimeVisualTransport, StateValue,
};
use hawk2ui_plugin_adapters::{
    ClapGuiParentHandle, ClapGuiWindowApi, ClapRuntimeEditorSession, PackageAdapterSet,
    PackageFormat, PackageRequest,
};
use hawk2ui_render::Color;
use hawk2ui_runtime::{
    RuntimeSceneBridge, RuntimeSceneFrame, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
    RuntimeVisual,
};
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use std::time::{SystemTime, UNIX_EPOCH};

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
    assert_eq!(adapter.drain_events().len(), 2);
}

#[test]
fn baseview_capabilities_advertise_only_parent_apis_the_backend_can_attach() {
    let capabilities = BaseviewCapabilities::plugin_editor();

    assert_eq!(
        capabilities.supported_clap_parent_apis(),
        [
            ClapGuiWindowApi::Win32,
            ClapGuiWindowApi::Cocoa,
            ClapGuiWindowApi::X11
        ]
    );
    assert!(capabilities.supports_clap_parent_api(ClapGuiWindowApi::Win32));
    assert!(capabilities.supports_clap_parent_api(ClapGuiWindowApi::Cocoa));
    assert!(capabilities.supports_clap_parent_api(ClapGuiWindowApi::X11));
    assert!(!capabilities.supports_clap_parent_api(ClapGuiWindowApi::Wayland));

    assert!(capabilities.supports_platform_handle(HostPlatformHandle::linux_x11(100, 200)));
    assert!(capabilities.supports_platform_handle(HostPlatformHandle::linux_x11_window(200)));
    assert!(capabilities.supports_platform_handle(HostPlatformHandle::linux_xcb(100, 200)));
    assert!(capabilities.supports_platform_handle(HostPlatformHandle::linux_xwayland(100, 200)));
    assert!(!capabilities.supports_platform_handle(HostPlatformHandle::linux_wayland(100, 200)));
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

    let x11_window_only =
        BaseviewNativeParent::try_from_handle(HostPlatformHandle::linux_x11_window(201))
            .expect("x11 window-only parent is supported");
    assert_eq!(
        x11_window_only.handle(),
        HostPlatformHandle::linux_x11_window(201)
    );
    assert_eq!(x11_window_only.backend(), BaseviewNativeParentBackend::X11);
    assert!(matches!(
        x11_window_only.raw_display_handle(),
        RawDisplayHandle::Xlib(display) if display.display.is_null()
    ));
    assert!(matches!(
        x11_window_only.raw_window_handle(),
        RawWindowHandle::Xlib(window) if window.window == 201
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
        HostPlatformHandle::linux_x11_window(0),
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
    assert!((adapter.open_options().size.width - 320.0).abs() < f64::EPSILON);
    assert!((adapter.open_options().size.height - 180.0).abs() < f64::EPSILON);
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
fn baseview_adapter_records_host_driven_show_hide_without_process_quit() {
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
fn baseview_event_translator_routes_native_window_keyboard_pointer_and_teardown_events() {
    use baseview::{
        Event, EventStatus, MouseButton, MouseEvent, Point, ScrollDelta, Size, WindowEvent,
        WindowInfo,
    };

    let mut translator = BaseviewEventTranslator::new(SurfaceMetrics::new(320.0, 180.0, 1.0));

    let resized = translator.translate(&Event::Window(WindowEvent::Resized(
        WindowInfo::from_logical_size(Size::new(640.0, 360.0), 2.0),
    )));
    assert_eq!(resized.status, EventStatus::Captured);
    assert!(
        resized
            .events
            .contains(&PluginHostEvent::HostResize(SurfaceMetrics::new(
                640.0, 360.0, 2.0
            )))
    );
    assert!(resized.events.contains(&PluginHostEvent::DpiChanged(2.0)));
    assert_eq!(translator.metrics(), SurfaceMetrics::new(640.0, 360.0, 2.0));

    let focused = translator.translate(&Event::Window(WindowEvent::Focused));
    assert_eq!(focused.events, [PluginHostEvent::FocusRouted(true)]);
    let unfocused = translator.translate(&Event::Window(WindowEvent::Unfocused));
    assert_eq!(unfocused.events, [PluginHostEvent::FocusRouted(false)]);

    let keyboard = translator.translate(&Event::Keyboard(keyboard_types::KeyboardEvent::default()));
    assert_eq!(
        keyboard.events,
        [PluginHostEvent::KeyboardRouted(KeyboardInput::new(
            "Unidentified",
            true
        ))]
    );
    assert_eq!(keyboard.status, EventStatus::Captured);

    let moved = translator.translate(&Event::Mouse(MouseEvent::CursorMoved {
        position: Point::new(10.0, 12.0),
        modifiers: keyboard_types::Modifiers::default(),
    }));
    assert_eq!(
        moved.events,
        [PluginHostEvent::PointerRouted(PointerInput::new(
            10.0, 12.0, "move"
        ))]
    );

    let pressed = translator.translate(&Event::Mouse(MouseEvent::ButtonPressed {
        button: MouseButton::Left,
        modifiers: keyboard_types::Modifiers::default(),
    }));
    assert_eq!(
        pressed.events,
        [PluginHostEvent::PointerRouted(PointerInput::new(
            10.0,
            12.0,
            "left-down"
        ))]
    );

    let extra_button = translator.translate(&Event::Mouse(MouseEvent::ButtonPressed {
        button: MouseButton::Other(8),
        modifiers: keyboard_types::Modifiers::default(),
    }));
    assert_eq!(
        extra_button.events,
        [PluginHostEvent::PointerRouted(PointerInput::new(
            10.0,
            12.0,
            "other-8-down"
        ))]
    );

    let wheel = translator.translate(&Event::Mouse(MouseEvent::WheelScrolled {
        delta: ScrollDelta::Lines { x: 1.0, y: -2.0 },
        modifiers: keyboard_types::Modifiers::default(),
    }));
    assert_eq!(
        wheel.events,
        [PluginHostEvent::PointerRouted(PointerInput::new(
            10.0,
            12.0,
            "wheel-lines:1:-2"
        ))]
    );

    let closing = translator.translate(&Event::Window(WindowEvent::WillClose));
    assert_eq!(
        closing.events,
        [
            PluginHostEvent::EditorDestroyed("baseview child window closed".into()),
            PluginHostEvent::SafeTeardownComplete,
        ]
    );
    let duplicate_close = translator.translate(&Event::Window(WindowEvent::WillClose));
    assert!(duplicate_close.events.is_empty());
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
    assert!(snapshot.pixels().contains(&0x000c_2238));
    assert_eq!(adapter.presented_frame_count(), 1);
    assert_eq!(
        adapter
            .last_presented_frame()
            .expect("presented snapshot is retained")
            .pixel_at(10, 10),
        Some(0x000c_2238)
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
    assert!(resized_snapshot.pixels().contains(&0x005a_781e));
    assert_eq!(adapter.presented_frame_count(), 2);
}

#[test]
fn baseview_adapter_renders_verified_clap_runtime_editor_session_frame() {
    let sealed_artifact = SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 320.0, "height": 180.0 },
        "root": {
            "id": "runtime-root",
            "width": 320.0,
            "height": 180.0,
            "visual": { "fill": [26, 111, 74, 255] },
            "children": [
                {
                    "id": "runtime-label",
                    "width": 160.0,
                    "height": 32.0,
                    "visual": {
                        "text": {
                            "value": "Runtime Editor",
                            "font_size": 16.0,
                            "color": [240, 245, 255, 255]
                        }
                    }
                }
            ]
        }
    }));
    let runtime_artifact =
        serde_json::to_value(&sealed_artifact).expect("sealed artifact serializes");
    let output_root = temp_package_root("hawk2ui-baseview-clap-runtime-editor");
    let request = PackageRequest::new(
        FormatMetadata::new(
            "com.hawk2ui.baseview-runtime",
            "Baseview Runtime",
            "Hawk2UI",
        ),
        BundleOutput::new(output_root.to_string_lossy(), "BaseviewRuntime"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(runtime_artifact)
    .with_format(PackageFormat::Clap);

    let outputs = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds")
        .materialize()
        .expect("materialization succeeds");
    let session = ClapRuntimeEditorSession::load_from_package(&outputs[0].output_path)
        .expect("verified CLAP runtime editor session loads");
    let host_config = session
        .baseview_host_config(
            ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
                .expect("CLAP parent handle validates"),
            Some(7),
        )
        .expect("Baseview host handoff builds");
    let parent = BaseviewParentFixture::from_platform_handle(
        "clap-runtime-parent",
        host_config.host_parent(),
    );
    let mut adapter = BaseviewPluginAdapter::attach(host_config.editor_config().clone(), parent)
        .expect("Baseview adapter accepts CLAP parent handoff");
    adapter.drain_events();

    let frame = session
        .runtime_scene_frame()
        .expect("runtime scene frame builds from sealed payload");
    let snapshot = adapter
        .render_scene_frame(&frame)
        .expect("sealed runtime frame renders through Baseview adapter");

    assert_eq!((snapshot.width(), snapshot.height()), (320, 180));
    assert_eq!(snapshot.pixel_at(10, 10), Some(0x001a_6f4a));
    assert!(snapshot.pixels().contains(&0x001a_6f4a));
    assert_eq!(adapter.presented_frame_count(), 1);
}

#[test]
fn baseview_clap_runtime_editor_attaches_presents_and_tears_down_live_session() {
    let sealed_artifact = SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 320.0, "height": 180.0 },
        "root": {
            "id": "runtime-root",
            "width": 320.0,
            "height": 180.0,
            "visual": { "fill": [26, 111, 74, 255] },
            "children": []
        }
    }));
    let runtime_artifact =
        serde_json::to_value(&sealed_artifact).expect("sealed artifact serializes");
    let output_root = temp_package_root("hawk2ui-baseview-clap-live-runtime-editor");
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.live-runtime", "Live Runtime", "Hawk2UI"),
        BundleOutput::new(output_root.to_string_lossy(), "LiveRuntime"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(runtime_artifact)
    .with_format(PackageFormat::Clap);
    let outputs = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds")
        .materialize()
        .expect("materialization succeeds");
    let session = ClapRuntimeEditorSession::load_from_package(&outputs[0].output_path)
        .expect("verified CLAP runtime editor session loads");

    let mut editor = BaseviewClapRuntimeEditor::attach(
        session,
        ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
            .expect("CLAP parent handle validates"),
        Some(7),
        "clap-live-runtime-parent",
    )
    .expect("live CLAP runtime editor attaches");
    editor.drain_events();

    let snapshot = editor
        .present_runtime_frame()
        .expect("live CLAP runtime editor presents sealed scene");
    assert_eq!((snapshot.width(), snapshot.height()), (320, 180));
    assert_eq!(snapshot.pixel_at(10, 10), Some(0x001a_6f4a));
    assert_eq!(editor.presented_frame_count(), 1);

    editor
        .try_host_resize(SurfaceMetrics::new(640.0, 360.0, 2.0))
        .expect("live editor accepts resize");
    let resized_snapshot = editor
        .present_runtime_frame()
        .expect("live CLAP runtime editor presents after resize");
    assert_eq!(
        (resized_snapshot.width(), resized_snapshot.height()),
        (1280, 720)
    );
    assert_eq!(editor.presented_frame_count(), 2);

    editor.hide_editor("host hid editor");
    editor.show_editor("host showed editor");
    editor.destroy_editor("host closed editor");
    let error = editor
        .present_runtime_frame()
        .expect_err("destroyed live editor must not render");
    assert_eq!(error.rule(), "baseview.editor.destroyed");
}

#[test]
fn baseview_clap_runtime_editor_host_drives_callback_lifecycle_from_plugin_path() {
    let sealed_artifact = SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 320.0, "height": 180.0 },
        "root": {
            "id": "runtime-root",
            "width": 320.0,
            "height": 180.0,
            "visual": { "fill": [26, 111, 74, 255] },
            "children": []
        }
    }));
    let (runtime_artifact, verifier) = signed_runtime_artifact_value(sealed_artifact);
    let output_root = temp_package_root("hawk2ui-baseview-clap-host-callback-lifecycle");
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.host-callback", "Host Callback", "Hawk2UI"),
        BundleOutput::new(output_root.to_string_lossy(), "HostCallback"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(runtime_artifact)
    .with_format(PackageFormat::Clap);
    let outputs = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds")
        .materialize()
        .expect("materialization succeeds");
    let clap_plugin_path = std::path::Path::new(&outputs[0].output_path).join("HostCallback.clap");
    let mut host = BaseviewClapRuntimeEditorHost::new(&clap_plugin_path, Some(7))
        .with_release_verifier(verifier);

    let error = host.show().expect_err("show before create must fail");
    assert_eq!(error.rule(), "baseview.clap-runtime-editor.not-attached");
    host.create(ClapGuiWindowApi::X11, false)
        .expect("host create resolves verified runtime session");
    assert!(host.created());
    assert!(!host.attached());

    host.set_parent(
        ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
            .expect("CLAP parent handle validates"),
        "clap-host-callback-parent",
    )
    .expect("host set-parent attaches live Baseview editor");
    assert!(host.attached());

    let snapshot = host.show().expect("host show presents runtime frame");
    assert_eq!((snapshot.width(), snapshot.height()), (320, 180));
    assert_eq!(snapshot.pixel_at(10, 10), Some(0x001a_6f4a));
    assert_eq!(host.presented_frame_count(), 1);
    assert!(host.visible());

    host.hide().expect("host hide delegates to live editor");
    assert!(!host.visible());
    host.destroy()
        .expect("host destroy delegates safe teardown");
    assert!(!host.created());
    assert!(!host.attached());
    let error = host.show().expect_err("show after destroy must fail");
    assert_eq!(error.rule(), "baseview.clap-runtime-editor.not-attached");
}

#[test]
fn baseview_clap_runtime_editor_host_requires_trusted_release_key() {
    let sealed_artifact = SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 320.0, "height": 180.0 },
        "root": {
            "id": "runtime-root",
            "width": 320.0,
            "height": 180.0,
            "visual": { "fill": [26, 111, 74, 255] },
            "children": []
        }
    }));
    let (runtime_artifact, verifier) = signed_runtime_artifact_value(sealed_artifact);
    let output_root = temp_package_root("hawk2ui-baseview-clap-host-trusted-release");
    let request = PackageRequest::new(
        FormatMetadata::new(
            "com.hawk2ui.host-trusted-release",
            "Host Trusted Release",
            "Hawk2UI",
        ),
        BundleOutput::new(output_root.to_string_lossy(), "HostTrustedRelease"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(runtime_artifact)
    .with_format(PackageFormat::Clap);
    let outputs = PackageAdapterSet::new()
        .plan(&request)
        .expect("trusted release package plan succeeds")
        .materialize()
        .expect("trusted release package materializes");
    let clap_plugin_path =
        std::path::Path::new(&outputs[0].output_path).join("HostTrustedRelease.clap");

    let mut untrusted_host = BaseviewClapRuntimeEditorHost::new(&clap_plugin_path, Some(7));
    let error = untrusted_host
        .create(ClapGuiWindowApi::X11, false)
        .expect_err("host create must reject packages signed by unknown keys");
    assert_eq!(
        error.rule(),
        "package.clap-runtime-editor.security.package.signature-invalid"
    );

    let mut trusted_host = BaseviewClapRuntimeEditorHost::new(&clap_plugin_path, Some(7))
        .with_release_verifier(verifier);
    trusted_host
        .create(ClapGuiWindowApi::X11, false)
        .expect("host create accepts trusted signed runtime package");
    assert!(trusted_host.created());
}

#[test]
#[allow(clippy::too_many_lines)]
fn baseview_clap_runtime_editor_host_tracks_parameter_and_state_events() {
    let sealed_artifact = SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 320.0, "height": 180.0 },
        "root": {
            "id": "runtime-root",
            "width": 320.0,
            "height": 180.0,
            "visual": { "fill": [26, 111, 74, 255] },
            "children": []
        }
    }));
    let (runtime_artifact, verifier) = signed_runtime_artifact_value(sealed_artifact);
    let output_root = temp_package_root("hawk2ui-baseview-clap-host-parameter-state");
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.host-state", "Host State", "Hawk2UI"),
        BundleOutput::new(output_root.to_string_lossy(), "HostState"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(runtime_artifact)
    .with_format(PackageFormat::Clap);
    let outputs = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds")
        .materialize()
        .expect("materialization succeeds");
    let clap_plugin_path = std::path::Path::new(&outputs[0].output_path).join("HostState.clap");
    let mut host = BaseviewClapRuntimeEditorHost::new(&clap_plugin_path, Some(7))
        .with_release_verifier(verifier);

    let error = host
        .apply_parameter_value("gain", ParameterValue::Float(0.5))
        .expect_err("parameter events before create are rejected");
    assert_eq!(error.rule(), "baseview.clap-runtime-editor.not-created");

    host.create(ClapGuiWindowApi::X11, false)
        .expect("host create resolves verified runtime session");
    host.apply_parameter_value("gain", ParameterValue::Float(0.5))
        .expect("created host accepts finite parameter value");
    host.apply_parameter_value("bypass", ParameterValue::Bool(true))
        .expect("created host accepts boolean parameter value");
    host.apply_parameter_value("mode", ParameterValue::Choice(2))
        .expect("created host accepts choice parameter value");
    host.apply_parameter_value("steps", ParameterValue::Int(4))
        .expect("created host accepts integer parameter value");
    assert_eq!(
        host.parameter_value("gain"),
        Some(&ParameterValue::Float(0.5))
    );

    host.set_parent(
        ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
            .expect("CLAP parent handle validates"),
        "clap-host-state-parent",
    )
    .expect("host set-parent attaches live Baseview editor");
    host.show().expect("host show presents runtime frame");
    host.hide().expect("host hide preserves parameter state");
    assert_eq!(
        host.parameter_value("bypass"),
        Some(&ParameterValue::Bool(true))
    );

    let saved = host.save_state().expect("host saves state after create");
    assert_eq!(
        saved.parameter_state.get("gain"),
        Some(&StateValue::Float(0.5))
    );
    assert_eq!(
        saved.parameter_state.get("bypass"),
        Some(&StateValue::Bool(true))
    );
    assert_eq!(
        saved.parameter_state.get("mode"),
        Some(&StateValue::Choice(2))
    );
    assert_eq!(
        saved.parameter_state.get("steps"),
        Some(&StateValue::Int(4))
    );

    let replacement = PluginStateEnvelope::new(1)
        .parameter("gain", StateValue::Float(0.75))
        .parameter("bypass", StateValue::Bool(false))
        .parameter("mode", StateValue::Choice(1))
        .parameter("steps", StateValue::Int(8));
    host.load_state(replacement)
        .expect("host loads parameter state after create");
    assert_eq!(
        host.parameter_value("gain"),
        Some(&ParameterValue::Float(0.75))
    );
    assert_eq!(
        host.parameter_value("bypass"),
        Some(&ParameterValue::Bool(false))
    );
    assert_eq!(
        host.parameter_value("mode"),
        Some(&ParameterValue::Choice(1))
    );
    assert_eq!(host.parameter_value("steps"), Some(&ParameterValue::Int(8)));

    let error = host
        .apply_parameter_value("gain", ParameterValue::Float(f64::NAN))
        .expect_err("non-finite float parameters are rejected");
    assert_eq!(
        error.rule(),
        "baseview.clap-runtime-editor.parameter-invalid"
    );

    host.destroy().expect("destroy succeeds");
    let error = host
        .save_state()
        .expect_err("state save after destroy is rejected");
    assert_eq!(error.rule(), "baseview.clap-runtime-editor.not-created");
}

#[test]
fn baseview_clap_runtime_editor_host_drains_realtime_visuals_with_frame_gate() {
    let sealed_artifact = SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 320.0, "height": 180.0 },
        "root": {
            "id": "runtime-root",
            "width": 320.0,
            "height": 180.0,
            "visual": { "fill": [26, 111, 74, 255] },
            "children": []
        }
    }));
    let (runtime_artifact, verifier) = signed_runtime_artifact_value(sealed_artifact);
    let output_root = temp_package_root("hawk2ui-baseview-clap-host-realtime");
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.host-realtime", "Host Realtime", "Hawk2UI"),
        BundleOutput::new(output_root.to_string_lossy(), "HostRealtime"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(runtime_artifact)
    .with_format(PackageFormat::Clap);
    let outputs = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds")
        .materialize()
        .expect("materialization succeeds");
    let clap_plugin_path = std::path::Path::new(&outputs[0].output_path).join("HostRealtime.clap");
    let mut host = BaseviewClapRuntimeEditorHost::new(&clap_plugin_path, Some(7))
        .with_release_verifier(verifier);
    let (mut writer, mut reader) =
        RealtimeVisualTransport::split_preallocated(4, FrameDropPolicy::DropNewest);
    let mut gate = RealtimeVisualFrameGate::new(60).expect("valid realtime frame gate");

    let error = host
        .drain_realtime_visuals(&mut reader, 0, &mut gate)
        .expect_err("realtime drains before attach are rejected");
    assert_eq!(error.rule(), "baseview.clap-runtime-editor.not-attached");

    host.create(ClapGuiWindowApi::X11, false)
        .expect("host create resolves verified runtime session");
    host.set_parent(
        ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
            .expect("CLAP parent handle validates"),
        "clap-host-realtime-parent",
    )
    .expect("host set-parent attaches live Baseview editor");

    let push = writer.audio_thread_push(RealtimeVisualPacket::meter("meter", 0.8));
    assert!(push.accepted);
    assert_eq!(push.dropped_frames, 0);
    assert_eq!(
        host.drain_realtime_visuals(&mut reader, 0, &mut gate)
            .expect("first realtime drain is due"),
        1
    );
    assert_eq!(
        host.latest_realtime_visual_packets(),
        std::slice::from_ref(&RealtimeVisualPacket::meter("meter", 0.8))
    );

    let _ = writer.audio_thread_push(RealtimeVisualPacket::analyzer("analyzer", &[0.1, 0.4]));
    assert_eq!(
        host.drain_realtime_visuals(&mut reader, 1, &mut gate)
            .expect("early realtime drain is gated"),
        0
    );
    assert_eq!(
        host.latest_realtime_visual_packets(),
        std::slice::from_ref(&RealtimeVisualPacket::meter("meter", 0.8))
    );
    assert_eq!(
        host.drain_realtime_visuals(&mut reader, 17, &mut gate)
            .expect("next realtime drain is due"),
        1
    );
    assert_eq!(
        host.latest_realtime_visual_packets(),
        std::slice::from_ref(&RealtimeVisualPacket::analyzer("analyzer", &[0.1, 0.4]))
    );

    host.destroy().expect("destroy succeeds");
    assert!(host.latest_realtime_visual_packets().is_empty());
}

#[test]
#[allow(clippy::too_many_lines)]
fn baseview_clap_runtime_editor_host_dispatches_typed_abi_commands() {
    let sealed_artifact = SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 320.0, "height": 180.0 },
        "root": {
            "id": "runtime-root",
            "width": 320.0,
            "height": 180.0,
            "visual": { "fill": [26, 111, 74, 255] },
            "children": []
        }
    }));
    let (runtime_artifact, verifier) = signed_runtime_artifact_value(sealed_artifact);
    let output_root = temp_package_root("hawk2ui-baseview-clap-host-command-dispatch");
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.host-command", "Host Command", "Hawk2UI"),
        BundleOutput::new(output_root.to_string_lossy(), "HostCommand"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(runtime_artifact)
    .with_format(PackageFormat::Clap);
    let outputs = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds")
        .materialize()
        .expect("materialization succeeds");
    let clap_plugin_path = std::path::Path::new(&outputs[0].output_path).join("HostCommand.clap");
    let mut host = BaseviewClapRuntimeEditorHost::new(&clap_plugin_path, Some(7))
        .with_release_verifier(verifier);

    assert_eq!(
        host.dispatch(BaseviewClapRuntimeEditorHostCommand::Create {
            api: ClapGuiWindowApi::X11,
            is_floating: false,
        })
        .expect("create dispatch succeeds"),
        BaseviewClapRuntimeEditorHostResponse::Created
    );
    assert_eq!(
        host.dispatch(BaseviewClapRuntimeEditorHostCommand::SetParent {
            parent: ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
                .expect("CLAP parent handle validates"),
            parent_fixture_id: "clap-host-command-parent",
        })
        .expect("set-parent dispatch succeeds"),
        BaseviewClapRuntimeEditorHostResponse::ParentAttached
    );
    assert_eq!(
        host.dispatch(BaseviewClapRuntimeEditorHostCommand::ApplyParameter {
            parameter_id: "gain".into(),
            value: ParameterValue::Float(0.25),
        })
        .expect("parameter dispatch succeeds"),
        BaseviewClapRuntimeEditorHostResponse::ParameterApplied
    );
    assert_eq!(
        host.dispatch(BaseviewClapRuntimeEditorHostCommand::Show)
            .expect("show dispatch succeeds"),
        BaseviewClapRuntimeEditorHostResponse::FramePresented {
            width: 320,
            height: 180,
            presented_frame_count: 1,
        }
    );
    let (mut writer, mut reader) =
        RealtimeVisualTransport::split_preallocated(4, FrameDropPolicy::DropNewest);
    let mut gate = RealtimeVisualFrameGate::new(60).expect("valid realtime frame gate");
    let _ = writer.audio_thread_push(RealtimeVisualPacket::meter("meter", 0.9));
    assert_eq!(
        host.dispatch_realtime_visuals(&mut reader, 0, &mut gate)
            .expect("realtime dispatch succeeds"),
        BaseviewClapRuntimeEditorHostResponse::RealtimeVisualsDrained { packet_count: 1 }
    );
    let saved = host
        .dispatch(BaseviewClapRuntimeEditorHostCommand::SaveState)
        .expect("save-state dispatch succeeds");
    assert!(matches!(
        saved,
        BaseviewClapRuntimeEditorHostResponse::StateSaved(_)
    ));
    assert_eq!(
        host.dispatch(BaseviewClapRuntimeEditorHostCommand::LoadState(
            PluginStateEnvelope::new(1).parameter("gain", StateValue::Float(0.5)),
        ))
        .expect("load-state dispatch succeeds"),
        BaseviewClapRuntimeEditorHostResponse::StateLoaded
    );
    assert_eq!(
        host.dispatch(BaseviewClapRuntimeEditorHostCommand::Hide)
            .expect("hide dispatch succeeds"),
        BaseviewClapRuntimeEditorHostResponse::Hidden
    );
    assert_eq!(
        host.dispatch(BaseviewClapRuntimeEditorHostCommand::Destroy)
            .expect("destroy dispatch succeeds"),
        BaseviewClapRuntimeEditorHostResponse::Destroyed
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn baseview_clap_runtime_editor_host_binds_generated_text_abi_to_live_editor() {
    let sealed_artifact = SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 320.0, "height": 180.0 },
        "root": {
            "id": "runtime-root",
            "width": 320.0,
            "height": 180.0,
            "visual": { "fill": [26, 111, 74, 255] },
            "children": []
        }
    }));
    let (runtime_artifact, verifier) = signed_runtime_artifact_value(sealed_artifact);
    let output_root = temp_package_root("hawk2ui-baseview-clap-host-text-abi");
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.host-text-abi", "Host Text ABI", "Hawk2UI"),
        BundleOutput::new(output_root.to_string_lossy(), "HostTextAbi"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(runtime_artifact)
    .with_format(PackageFormat::Clap);
    let outputs = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds")
        .materialize()
        .expect("materialization succeeds");
    let clap_plugin_path = std::path::Path::new(&outputs[0].output_path).join("HostTextAbi.clap");
    let mut host = BaseviewClapRuntimeEditorHost::new(&clap_plugin_path, Some(7))
        .with_release_verifier(verifier);
    let bridge = BaseviewClapRuntimeEditorHostAbiBridge::new();

    assert!(
        bridge
            .abi_contract()
            .contains("function=hawk2ui_editor_dispatch")
    );
    assert!(
        bridge
            .abi_contract()
            .contains("command=drain_realtime_visuals")
    );
    assert_eq!(
        bridge
            .dispatch_text(&mut host, "command=create\napi=x11\nfloating=false\n")
            .expect("ABI create dispatch succeeds"),
        "response=created\n"
    );
    assert_eq!(
        bridge
            .dispatch_text(&mut host, "command=set_parent\napi=x11\nparent=42\n")
            .expect("ABI set-parent dispatch succeeds"),
        "response=parent_attached\n"
    );
    assert_eq!(
        bridge
            .dispatch_text(
                &mut host,
                "command=apply_parameter\nparameter_id=gain\nvalue=0.25\n",
            )
            .expect("ABI parameter dispatch succeeds"),
        "response=parameter_applied\n"
    );
    assert_eq!(
        bridge
            .dispatch_text(
                &mut host,
                "command=apply_parameter\nparameter_id=bypass\nbool=true\n",
            )
            .expect("ABI bool parameter dispatch succeeds"),
        "response=parameter_applied\n"
    );
    assert_eq!(
        bridge
            .dispatch_text(
                &mut host,
                "command=apply_parameter\nparameter_id=mode\nchoice=2\n",
            )
            .expect("ABI choice parameter dispatch succeeds"),
        "response=parameter_applied\n"
    );
    assert_eq!(
        bridge
            .dispatch_text(
                &mut host,
                "command=apply_parameter\nparameter_id=steps\nint=4\n",
            )
            .expect("ABI integer parameter dispatch succeeds"),
        "response=parameter_applied\n"
    );
    let show = bridge
        .dispatch_text(&mut host, "command=show\n")
        .expect("ABI show dispatch succeeds");
    assert!(show.contains("response=frame_presented"));
    assert!(show.contains("width=320"));
    assert!(show.contains("height=180"));
    assert_eq!(host.presented_frame_count(), 1);

    let saved = bridge
        .dispatch_text(&mut host, "command=save_state\n")
        .expect("ABI save-state dispatch succeeds");
    assert!(saved.contains("response=state_saved"));
    assert!(saved.contains("param.gain.bits="));
    assert!(saved.contains("param.bypass.bool=true"));
    assert!(saved.contains("param.mode.choice=2"));
    assert!(saved.contains("param.steps.int=4"));
    let load = format!(
        "command=load_state\nparam.gain.bits={}\nparam.bypass.bool=false\nparam.mode.choice=1\nparam.steps.int=8\n",
        0.75f64.to_bits()
    );
    assert_eq!(
        bridge
            .dispatch_text(&mut host, &load)
            .expect("ABI load-state dispatch succeeds"),
        "response=state_loaded\n"
    );
    assert_eq!(
        host.parameter_value("gain"),
        Some(&ParameterValue::Float(0.75))
    );
    assert_eq!(
        host.parameter_value("bypass"),
        Some(&ParameterValue::Bool(false))
    );
    assert_eq!(
        host.parameter_value("mode"),
        Some(&ParameterValue::Choice(1))
    );
    assert_eq!(host.parameter_value("steps"), Some(&ParameterValue::Int(8)));
    let (mut writer, mut reader) =
        RealtimeVisualTransport::split_preallocated(4, FrameDropPolicy::DropNewest);
    let mut gate = RealtimeVisualFrameGate::new(60).expect("valid realtime frame gate");
    let _ = writer.audio_thread_push(RealtimeVisualPacket::meter("meter", 0.8));
    let realtime = bridge
        .dispatch_text_with_realtime(
            &mut host,
            "command=drain_realtime_visuals\ntimestamp_ms=0\n",
            &mut reader,
            &mut gate,
        )
        .expect("ABI realtime dispatch succeeds");
    assert_eq!(
        realtime,
        "response=realtime_visuals_drained\npacket_count=1\n"
    );
    assert_eq!(
        host.latest_realtime_visual_packets(),
        std::slice::from_ref(&RealtimeVisualPacket::meter("meter", 0.8))
    );
    assert_eq!(
        bridge
            .dispatch_text(&mut host, "command=hide\n")
            .expect("ABI hide dispatch succeeds"),
        "response=hidden\n"
    );
    assert_eq!(
        bridge
            .dispatch_text(&mut host, "command=destroy\n")
            .expect("ABI destroy dispatch succeeds"),
        "response=destroyed\n"
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

fn temp_package_root(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn signed_runtime_artifact_value(
    artifact: SealedArtifact,
) -> (serde_json::Value, ArtifactSignatureVerifier) {
    let signing_key = ArtifactSigningKey::ed25519_sha256_v1("baseview-test-release-key", [11; 32]);
    let signed_artifact =
        signing_key.sign(&artifact.with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"baseview-runtime-script"),
        )));
    (
        serde_json::to_value(&signed_artifact).expect("signed artifact serializes"),
        ArtifactSignatureVerifier::new([signing_key.verification_key()]),
    )
}

const VALID_PLUGIN_MANIFEST: &str = r#"
[identity]
id = "com.hawk2ui.baseview-runtime"
name = "Baseview Runtime"
version = "0.1.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["plugin-editor"]

[[targets]]
kind = "plugin"
name = "clap"

[plugin]
id = "com.hawk2ui.baseview-runtime"
name = "Baseview Runtime"

[editor]
width = 320
height = 180
"#;

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
