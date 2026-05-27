use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use hawk2ui_assets::{AssetBackend, AssetHash, AssetLimits};
use hawk2ui_host::{DesktopWindowConfig, SurfaceMetrics};
use hawk2ui_host_winit::{
    DesktopRuntimeEvent, SoftwareFrameRenderer, WinitDesktopRuntimeConfig, WinitHostError,
};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_render::{Color, CustomSurfaceCategory, CustomSurfaceDataSnapshot};
use hawk2ui_runtime::{
    RuntimeCustomSurfaceVisual, RuntimeSceneBridge, RuntimeViewId, RuntimeViewNode,
    RuntimeViewTree, RuntimeVisual,
};
use image::{ColorType, ImageEncoder};

#[test]
fn software_frame_renders_visible_pixels() {
    let renderer = SoftwareFrameRenderer::default();
    let pixels = renderer
        .render_frame("Hawk2UI", 96, 64, 1.25)
        .expect("software frame should render");

    assert_eq!(pixels.width(), 96);
    assert_eq!(pixels.height(), 64);
    assert_eq!(pixels.pixels().len(), 96 * 64);
    assert!(pixels.pixels().iter().all(|pixel| *pixel != 0));
    assert!(
        pixels
            .pixels()
            .iter()
            .any(|pixel| *pixel != pixels.pixels()[0])
    );
}

#[test]
fn winit_host_error_converts_to_shared_diagnostic() {
    let error = WinitHostError::new("desktop.failed", "desktop failed");
    let diagnostic = Diagnostic::from(error);

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "desktop.failed");
    assert_eq!(diagnostic.message, "desktop failed");
}

#[test]
fn software_frame_renders_runtime_scene_commands() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(32.0, 24.0)),
        RuntimeVisual::Fill(Color::rgba(255, 0, 0, 255)),
    ));
    let frame = RuntimeSceneBridge::new(Viewport::new(32.0, 24.0))
        .build(&tree)
        .expect("runtime scene frame should build");

    let pixels = SoftwareFrameRenderer::default()
        .render_scene_frame(&frame, 32, 24, 1.0)
        .expect("runtime scene should render to software frame");

    assert_eq!(pixels.width(), 32);
    assert_eq!(pixels.height(), 24);
    assert_eq!(pixels.pixels()[0], 0x00ff0000);
    assert!(pixels.pixels().iter().all(|pixel| *pixel == 0x00ff0000));
}

#[test]
fn software_frame_renders_runtime_custom_surface_commands() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(128.0, 64.0)),
        RuntimeVisual::None,
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("meter"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(96.0, 24.0)),
            RuntimeVisual::CustomSurface(
                RuntimeCustomSurfaceVisual::new(CustomSurfaceCategory::Meter).with_data_snapshot(
                    CustomSurfaceDataSnapshot::new([0.0, 0.5, 1.0]).expect("valid samples"),
                ),
            ),
        ),
    )
    .expect("custom surface attaches");
    let frame = RuntimeSceneBridge::new(Viewport::new(128.0, 64.0))
        .build(&tree)
        .expect("runtime scene frame should build");

    let pixels = SoftwareFrameRenderer::default()
        .render_scene_frame(&frame, 128, 64, 1.0)
        .expect("runtime custom surface should render to software frame");

    assert!(
        pixels.pixels().iter().any(|pixel| *pixel != 0x00000000),
        "custom surface should draw visible non-transparent pixels"
    );
}

#[test]
fn software_frame_uses_renderer_custom_surface_semantics() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(128.0, 64.0)),
        RuntimeVisual::None,
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("analyzer"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(96.0, 32.0)),
            RuntimeVisual::CustomSurface(
                RuntimeCustomSurfaceVisual::new(CustomSurfaceCategory::Analyzer)
                    .with_data_snapshot(
                        CustomSurfaceDataSnapshot::new([0.0, 0.25, 1.0, 0.4])
                            .expect("valid samples"),
                    ),
            ),
        ),
    )
    .expect("custom surface attaches");
    let frame = RuntimeSceneBridge::new(Viewport::new(128.0, 64.0))
        .build(&tree)
        .expect("runtime scene frame should build");

    let pixels = SoftwareFrameRenderer::default()
        .render_scene_frame(&frame, 128, 64, 1.0)
        .expect("runtime custom surface should render to software frame");

    assert!(
        pixels.pixels().iter().any(|pixel| {
            let red = (pixel >> 16) & 0xff;
            let green = (pixel >> 8) & 0xff;
            let blue = pixel & 0xff;
            blue >= 200 && green >= 120 && red <= 120
        }),
        "software host frames must use the Skia renderer's analyzer custom-surface styling"
    );
}

#[test]
fn software_frame_degrades_missing_runtime_assets_to_visible_placeholders() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(128.0, 96.0)),
        RuntimeVisual::None,
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("image"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(64.0, 32.0)),
            RuntimeVisual::ImageAsset("hero".to_string()),
        ),
    )
    .expect("image attaches")
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("vector"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(64.0, 32.0)),
            RuntimeVisual::VectorAsset("logo".to_string()),
        ),
    )
    .expect("vector attaches");
    let frame = RuntimeSceneBridge::new(Viewport::new(128.0, 96.0))
        .build(&tree)
        .expect("runtime scene frame should build");

    let pixels = SoftwareFrameRenderer::default()
        .render_scene_frame(&frame, 128, 96, 1.0)
        .expect("missing assets should degrade without failing the frame");

    assert!(pixels.pixels().iter().any(|pixel| *pixel != 0x00000000));
}

#[test]
fn software_frame_renders_registered_runtime_assets() {
    let mut assets = AssetBackend::new(AssetLimits::default());
    let image_bytes = png_1x1();
    let image = assets
        .compile_image(
            "hero",
            "assets/hero.png",
            &image_bytes,
            &AssetHash::sha256_bytes(&image_bytes),
        )
        .expect("image compiles");
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 16"><path d="M0 0 L32 0 L32 16 L0 16 Z"/></svg>"#;
    let vector = assets
        .compile_vector(
            "logo",
            "assets/logo.svg",
            svg,
            &AssetHash::sha256_bytes(svg),
        )
        .expect("vector compiles");
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(96.0, 64.0)),
        RuntimeVisual::None,
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("image"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(32.0, 16.0)),
            RuntimeVisual::ImageAsset("hero".to_string()),
        ),
    )
    .expect("image attaches")
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("vector"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(32.0, 16.0)),
            RuntimeVisual::VectorAsset("logo".to_string()),
        ),
    )
    .expect("vector attaches");
    let frame = RuntimeSceneBridge::new(Viewport::new(96.0, 64.0))
        .build(&tree)
        .expect("runtime scene frame should build");

    let pixels = SoftwareFrameRenderer::new()
        .with_assets([image, vector])
        .render_scene_frame(&frame, 96, 64, 1.0)
        .expect("registered runtime assets should render to software frame");

    assert!(
        pixels.pixels().iter().any(|pixel| *pixel == 0x00ff0000),
        "registered image asset should render decoded pixels, not a placeholder"
    );
    assert!(
        pixels.pixels().iter().any(|pixel| *pixel == 0x00ffffff),
        "registered vector asset should render lowered vector pixels, not a placeholder"
    );
}

#[test]
fn runtime_config_rejects_zero_size() {
    let config = WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
        "app",
        SurfaceMetrics::new(0.0, 480.0, 1.0),
    ));

    let error = config.validate().expect_err("zero width must fail");

    assert_eq!(error.rule(), "desktop.window.invalid-size");
}

fn png_1x1() -> Vec<u8> {
    let mut bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .write_image(&[255, 0, 0, 255], 1, 1, ColorType::Rgba8.into())
        .expect("test PNG encodes");
    bytes
}

#[test]
fn runtime_config_accepts_first_frame_smoke_mode() {
    let scene_tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(640.0, 480.0)),
        RuntimeVisual::Fill(Color::rgba(8, 10, 14, 255)),
    ));
    let config = WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
        "app",
        SurfaceMetrics::new(640.0, 480.0, 1.0),
    ))
    .with_runtime_tree(scene_tree)
    .with_exit_after_first_frame(true);

    config.validate().expect("valid runtime config");
    assert!(config.exit_after_first_frame());
    assert!(config.runtime_tree().is_some());
}

#[test]
fn runtime_config_accepts_animation_cadence_policy() {
    let policy = hawk2ui_runtime::AnimationCadencePolicy::new(30)
        .expect("30hz animation policy is valid")
        .with_reduced_rate_divisor(2)
        .expect("reduced-rate divisor is valid");
    let config = WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
        "app",
        SurfaceMetrics::new(640.0, 480.0, 1.0),
    ))
    .with_animation_policy(policy);

    config.validate().expect("valid runtime config");
    assert_eq!(config.animation_policy().max_frame_rate_hz(), Some(30));
    assert_eq!(config.animation_policy().reduced_rate_divisor(), 2);
}

#[test]
fn runtime_events_request_repaint_after_resize() {
    assert!(
        DesktopRuntimeEvent::Resized {
            physical_width: 1280,
            physical_height: 720,
            scale_factor: 1.0,
        }
        .requires_full_repaint()
    );
    assert!(DesktopRuntimeEvent::DpiChanged { scale_factor: 2.0 }.requires_full_repaint());
    assert!(!DesktopRuntimeEvent::KeyboardInput.requires_full_repaint());
}
