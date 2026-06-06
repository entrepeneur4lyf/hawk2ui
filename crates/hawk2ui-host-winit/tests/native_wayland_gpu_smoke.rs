#![cfg(target_os = "linux")]

use hawk2ui_host::{DesktopWindowConfig, SurfaceMetrics};
use hawk2ui_host_winit::{
    WinitDesktopRuntime, WinitDesktopRuntimeConfig, WinitPresentationBackend,
    WinitPresentationBackendUsed,
};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle};
use hawk2ui_render::Color;
use hawk2ui_runtime::{RuntimeViewId, RuntimeViewNode, RuntimeViewTree, RuntimeVisual};

#[test]
fn wayland_gpu_runtime_opens_renders_reads_back_and_exits_when_enabled() {
    if std::env::var("HAWK2UI_NATIVE_WAYLAND_GPU_SMOKE")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!("skipping native Wayland GPU smoke; set HAWK2UI_NATIVE_WAYLAND_GPU_SMOKE=1");
        return;
    }
    assert!(
        std::env::var("WAYLAND_DISPLAY").is_ok(),
        "native Wayland GPU smoke requires WAYLAND_DISPLAY"
    );

    let config = WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
        "hawk2ui-wayland-gpu-smoke",
        SurfaceMetrics::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_tree(two_band_tree())
    .with_presentation_backend(WinitPresentationBackend::GpuRequired)
    .with_exit_after_first_frame(true);

    let summary = WinitDesktopRuntime::new()
        .run_wayland_any_thread_blocking(config)
        .expect("Wayland GPU runtime should open, render, read back, and exit");

    assert!(summary.window_created);
    assert_eq!(
        summary.presentation_backend_used,
        WinitPresentationBackendUsed::Gpu
    );
    assert!(summary.frames_presented >= 1);
    assert!(summary.gpu_frames_presented >= 1);
    assert!(summary.gpu_readback_verified);
    assert!(!summary.close_requested);
}

fn two_band_tree() -> RuntimeViewTree {
    let root_id = RuntimeViewId::new("root");
    let top_id = RuntimeViewId::new("top");
    let bottom_id = RuntimeViewId::new("bottom");
    let root = RuntimeViewNode::new(
        root_id.clone(),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 180.0)),
        RuntimeVisual::Fill(Color::rgba(0, 0, 0, 255)),
    );
    let top = RuntimeViewNode::new(
        top_id,
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 90.0)),
        RuntimeVisual::Fill(Color::rgba(200, 40, 40, 255)),
    );
    let bottom = RuntimeViewNode::new(
        bottom_id,
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 90.0)),
        RuntimeVisual::Fill(Color::rgba(40, 40, 200, 255)),
    );

    RuntimeViewTree::new(root)
        .with_child(&root_id, top)
        .expect("top child attaches")
        .with_child(&root_id, bottom)
        .expect("bottom child attaches")
}
