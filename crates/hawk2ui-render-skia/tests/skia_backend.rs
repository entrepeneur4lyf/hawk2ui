use hawk2ui_assets::{AssetBackend, AssetHash, AssetLimits};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_render::{
    BackendCapabilities, Color, CustomDrawSurface, CustomSurfaceCategory,
    CustomSurfaceDataSnapshot, CustomSurfaceDrawRequest, CustomSurfaceFrameContext, Geometry,
    RendererBackend, ShaderEffectUniform, Transform,
};
use hawk2ui_render_skia::{
    SkiaBlendMode, SkiaImageDrawOptions, SkiaImageSampling, SkiaImageTileMode, SkiaRendererBackend,
    SkiaRuntimeEffectChildInput, SkiaRuntimeEffectUniform, SkiaSurfaceConfig, SkiaTextDrawOptions,
};
use hawk2ui_runtime::{
    RuntimeCustomSurfaceVisual, RuntimeSceneBridge, RuntimeShaderEffectVisual, RuntimeViewId,
    RuntimeViewNode, RuntimeViewTree, RuntimeVisual,
};
use hawk2ui_text::{FontCatalog, LineBreakMode, TextBackend, TextLayoutInput};
use image::{ColorType, ImageEncoder};

#[test]
fn skia_backend_matches_recording_backend_for_core_frame_commands() {
    let capabilities = BackendCapabilities::new()
        .with_gpu(false)
        .with_text(true)
        .with_images(true)
        .with_runtime_effects(true);
    let mut recording = hawk2ui_render::RecordingBackend::new(capabilities);
    let mut skia = SkiaRendererBackend::new();
    skia.register_image_asset("hero", ONE_BY_ONE_PNG).unwrap();

    drive_core_frame(&mut recording);
    drive_core_frame(&mut skia);

    assert_eq!(&skia.command_keys()[1..], recording.command_keys());
    assert_eq!(skia.dirty_regions(), recording.dirty_regions());
    assert_eq!(skia.capabilities(), capabilities);
}

#[test]
fn skia_backend_tracks_surface_resize_dpi_frame_and_dirty_state() {
    let mut backend = SkiaRendererBackend::new();

    backend
        .create_surface_with_config(
            SkiaSurfaceConfig::cpu_raster("main", 640, 360).with_dpi_scale(1.5),
        )
        .expect("surface creation succeeds");
    backend.resize_surface("main", 800, 600, 2.0).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .mark_dirty(Geometry::new(4.0, 8.0, 120.0, 32.0))
        .unwrap();
    backend.end_frame("main").unwrap();

    let surface = backend.surface("main").expect("surface exists");
    assert_eq!(surface.width(), 800);
    assert_eq!(surface.height(), 600);
    assert!((surface.dpi_scale() - 2.0).abs() < f32::EPSILON);
    assert_eq!(surface.pixel_width(), 1600);
    assert_eq!(surface.pixel_height(), 1200);
    assert_eq!(surface.presented_frames(), 1);
    assert_eq!(surface.dirty_regions().len(), 1);
    assert!(!surface.frame_active());
}

#[test]
fn skia_backend_rejects_oversized_surface_allocation() {
    // Surface dimensions are caller-influenced and only checked non-zero upstream, so without a
    // ceiling a finite-but-enormous request allocates an unbounded N32 raster surface (memory
    // exhaustion), with `capture_frame_snapshot` then allocating ~2x more. An over-cap physical
    // dimension must be refused *before* allocation. The small height keeps this test's pre-cap
    // allocation cheap while the width alone trips the bound.
    let mut backend = SkiaRendererBackend::new();
    let create_error = backend
        .create_surface_with_config(SkiaSurfaceConfig::cpu_raster("huge", 20_000, 64))
        .expect_err("an over-cap surface dimension is rejected before allocation");
    assert_eq!(create_error.diagnostic().rule(), "skia.surface.too-large");

    // The bound is on the *physical* (DPI-scaled) dimension: a modest logical size at a large DPI
    // is also rejected (9000 x 2.0 = 18000 physical).
    backend
        .create_surface_with_config(SkiaSurfaceConfig::cpu_raster("ok", 1280, 64))
        .expect("an in-bounds surface still allocates");
    let resize_error = backend
        .resize_surface("ok", 9000, 64, 2.0)
        .expect_err("an over-cap resize is rejected on the scaled dimension");
    assert_eq!(resize_error.diagnostic().rule(), "skia.surface.too-large");
}

#[test]
fn frame_snapshot_reads_presented_pixels_and_enforces_lifecycle() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 16, 16).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(0, 0, 0, 255))
        .unwrap();
    backend
        .fill(
            Geometry::new(2.0, 2.0, 4.0, 4.0),
            hawk2ui_render::Color::rgba(255, 0, 0, 255),
        )
        .unwrap();

    let active_error = backend
        .frame_snapshot("main")
        .expect_err("active frames must not expose a presented snapshot");

    assert_eq!(active_error.diagnostic().rule(), "skia.frame.active");

    backend.end_frame("main").unwrap();
    let snapshot = backend
        .frame_snapshot("main")
        .expect("snapshot is available after presentation");

    assert_eq!(snapshot.width(), 16);
    assert_eq!(snapshot.height(), 16);
    assert_eq!(snapshot.pixels().len(), 16 * 16);
    assert_eq!(snapshot.pixel_at(0, 0), Some(0x0000_0000));
    assert_eq!(snapshot.pixel_at(3, 3), Some(0x00ff_0000));
    assert_eq!(
        backend.surface("main").unwrap().presented_frames(),
        1,
        "ending a frame presents exactly one frame"
    );
}

#[test]
fn skia_backend_applies_full_affine_transforms() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 48, 32).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(0, 0, 0, 255))
        .unwrap();
    backend
        .push_transform(Transform::affine(2.0, 0.0, 0.0, 1.0, 8.0, 4.0))
        .unwrap();
    backend
        .fill(
            Geometry::new(2.0, 2.0, 6.0, 6.0),
            hawk2ui_render::Color::rgba(255, 0, 0, 255),
        )
        .unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();

    assert_eq!(snapshot.pixel_at(5, 5), Some(0x0000_0000));
    assert_eq!(snapshot.pixel_at(13, 7), Some(0x00ff_0000));
}

#[test]
fn clip_path_restricts_subsequent_draws_to_vector_geometry() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 24, 24).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(0, 0, 0, 255)).unwrap();
    backend
        .push_clip_path("M0 0 L20 0 L0 20 Z")
        .expect("clip path is accepted");
    backend
        .fill(
            Geometry::new(0.0, 0.0, 24.0, 24.0),
            Color::rgba(255, 0, 0, 255),
        )
        .unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();
    assert_eq!(snapshot.pixel_at(2, 2), Some(0x00ff_0000));
    assert_eq!(snapshot.pixel_at(22, 22), Some(0x0000_0000));
    assert!(
        backend
            .command_keys()
            .contains(&"clip-path:M0 0 L20 0 L0 20 Z".to_string())
    );
}

#[test]
fn blend_mode_rect_composites_with_existing_pixels() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 8, 8).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(255, 0, 0, 255)).unwrap();
    backend
        .draw_blended_rect(
            Geometry::new(0.0, 0.0, 8.0, 8.0),
            Color::rgba(0, 0, 255, 255),
            SkiaBlendMode::Plus,
        )
        .expect("blend draw succeeds");
    backend.end_frame("main").unwrap();

    assert_eq!(
        backend.frame_snapshot("main").unwrap().pixel_at(4, 4),
        Some(0x00ff_00ff)
    );
    assert!(
        backend
            .command_keys()
            .contains(&"blend-rect:0,0,8,8:Plus".to_string())
    );
}

#[test]
fn placed_text_and_images_render_into_target_regions() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 128, 72).unwrap();
    backend
        .register_image_asset("hero", ONE_BY_ONE_PNG)
        .unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(8, 10, 14, 255))
        .unwrap();
    backend
        .draw_text_at(
            "Placed",
            18.0,
            42.0,
            18.0,
            hawk2ui_render::Color::rgba(240, 245, 255, 255),
        )
        .unwrap();
    backend
        .draw_image_rect("hero", Geometry::new(84.0, 24.0, 24.0, 24.0))
        .unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();

    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(16.0, 22.0, 72.0, 28.0)) > 0,
        "placed text must affect pixels near its requested baseline"
    );
    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(84.0, 24.0, 24.0, 24.0)) > 0,
        "target-rectangle image draw must affect pixels in the requested rectangle"
    );
    assert_eq!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(0.0, 0.0, 8.0, 8.0)),
        0,
        "unaffected background corner should remain unchanged"
    );
}

#[test]
fn image_draw_options_support_source_rect_sampling_and_tiling() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 8, 8).unwrap();
    backend
        .register_image_asset("stripe", &png_rgba(&[255, 0, 0, 255, 0, 0, 255, 255], 2, 1))
        .unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(0, 0, 0, 255)).unwrap();
    backend
        .draw_image_rect_with_options(
            "stripe",
            Geometry::new(0.0, 0.0, 4.0, 4.0),
            SkiaImageDrawOptions::new()
                .with_source_rect(Geometry::new(1.0, 0.0, 1.0, 1.0))
                .with_sampling(SkiaImageSampling::Nearest),
        )
        .expect("source rect image draw succeeds");
    backend
        .draw_image_rect_with_options(
            "stripe",
            Geometry::new(0.0, 6.0, 4.0, 1.0),
            SkiaImageDrawOptions::new()
                .with_sampling(SkiaImageSampling::Nearest)
                .with_tile_modes(SkiaImageTileMode::Repeat, SkiaImageTileMode::Clamp),
        )
        .expect("tiled image draw succeeds");
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();
    assert_eq!(snapshot.pixel_at(1, 1), Some(0x0000_00ff));
    assert_eq!(snapshot.pixel_at(0, 6), Some(0x00ff_0000));
    assert_eq!(snapshot.pixel_at(1, 6), Some(0x0000_00ff));
    assert_eq!(snapshot.pixel_at(2, 6), Some(0x00ff_0000));
    assert_eq!(snapshot.pixel_at(3, 6), Some(0x0000_00ff));
}

#[test]
fn trait_draw_text_uses_configured_default_text_style() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 128, 72).unwrap();
    backend
        .set_default_text_style(18.0, 42.0, 18.0, Color::rgba(240, 245, 255, 255))
        .unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(8, 10, 14, 255)).unwrap();
    backend.draw_text("Trait").unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();

    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(16.0, 22.0, 72.0, 28.0)) > 0,
        "trait-level text draw must use configured visible placement and color"
    );
}

#[test]
fn text_draw_options_render_highlight_decorations_stroke_and_subpixel_command() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 128, 48).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(0, 0, 0, 255)).unwrap();
    backend
        .draw_text_at_with_options(
            "Hi",
            8.0,
            26.0,
            20.0,
            Color::rgba(255, 255, 255, 255),
            SkiaTextDrawOptions::new()
                .with_highlight(Color::rgba(32, 64, 160, 255))
                .with_stroke(Color::rgba(0, 0, 0, 255), 1.0)
                .with_underline(Color::rgba(0, 255, 0, 255), 2.0)
                .with_strikethrough(Color::rgba(255, 0, 0, 255), 2.0)
                .with_subpixel(true),
        )
        .expect("optioned text draw succeeds");
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();
    assert!(
        count_changed_pixels(snapshot, 0x0000_0000, Geometry::new(8.0, 6.0, 28.0, 20.0)) > 0,
        "highlight must paint behind the text"
    );
    assert_eq!(snapshot.pixel_at(10, 28), Some(0x0000_ff00));
    assert_eq!(snapshot.pixel_at(10, 19), Some(0x00ff_0000));
    assert!(backend.command_keys().iter().any(|key| {
        key.starts_with("text-at-options:Hi:8,26:20")
            && key.contains("stroke=true")
            && key.contains("underline=true")
            && key.contains("strike=true")
            && key.contains("subpixel=true")
    }));
}

#[test]
fn shaped_text_layout_renders_positioned_lines() {
    let text = TextBackend::new(
        FontCatalog::new()
            .with_system_family("Display")
            .with_fallback_family("Sans"),
    );
    let layout = text
        .layout(
            &TextLayoutInput::new("Gain reduction meter שלום", "Display", 18.0)
                .with_dpi_scale(1.25)
                .with_bidi(true)
                .with_line_break(LineBreakMode::Wrap {
                    max_width_px: 104.0,
                }),
        )
        .unwrap();
    assert!(layout.line_count() > 1);
    assert!(layout.bidi_resolved());
    assert!(layout.parley_processed());

    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 180, 120).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(8, 10, 14, 255)).unwrap();
    backend
        .draw_text_layout(&layout, 18.0, 18.0, Color::rgba(240, 245, 255, 255))
        .unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();

    assert!(
        count_changed_pixels(
            snapshot,
            0x0008_0a0e,
            Geometry::new(16.0, 22.0, 128.0, 72.0)
        ) > 0,
        "shaped layout lines must affect pixels in their measured region"
    );
    assert!(backend.command_keys().iter().any(|key| {
        key.starts_with("text-layout:Display:18,18:lines=")
            && key.contains(":bidi=true:")
            && key.contains(":parley=true:")
    }));
}

#[test]
fn registered_vector_assets_render_pixels_through_trait_draw_vector() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 64, 48).unwrap();
    backend
        .register_vector_paths("logo", ["M10 10 L30 10 L30 30 L10 30 Z"])
        .unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(8, 10, 14, 255)).unwrap();
    backend.draw_vector("logo").unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();

    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(10.0, 10.0, 20.0, 20.0)) > 0,
        "registered vector asset must render pixels in its path bounds"
    );
    assert!(backend.command_keys().contains(&"vector:logo".to_string()));
}

#[test]
fn compiled_asset_records_register_and_render_image_and_vector_pixels() {
    let mut assets = AssetBackend::new(AssetLimits::default());
    let image_bytes = png_1x1();
    let image = assets
        .compile_image(
            "hero",
            "assets/hero.png",
            &image_bytes,
            &AssetHash::sha256_bytes(&image_bytes),
        )
        .unwrap();
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 48"><path fill="#ff0000" d="M10 10 L30 10 L30 30 L10 30 Z"/></svg>"##;
    let vector = assets
        .compile_vector(
            "logo",
            "assets/logo.svg",
            svg,
            &AssetHash::sha256_bytes(svg),
        )
        .unwrap();

    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 80, 48).unwrap();
    backend.register_compiled_asset(&image).unwrap();
    backend.register_compiled_asset(&vector).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(8, 10, 14, 255)).unwrap();
    backend
        .draw_image_rect("hero", Geometry::new(40.0, 8.0, 16.0, 16.0))
        .unwrap();
    backend.draw_vector("logo").unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();

    assert!(count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(40.0, 8.0, 16.0, 16.0)) > 0);
    assert!(count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(10.0, 10.0, 20.0, 20.0)) > 0);
    assert_eq!(
        snapshot.pixel_at(16, 16),
        Some(0x00ff_0000),
        "compiled vector fill color must survive asset lowering and Skia registration"
    );
}

#[test]
fn compiled_vector_style_paint_survives_asset_lowering_and_skia_registration() {
    let mut assets = AssetBackend::new(AssetLimits::default());
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 48"><path style="fill:#00cc66" d="M10 10 L30 10 L30 30 L10 30 Z"/></svg>"#;
    let vector = assets
        .compile_vector(
            "styled-logo",
            "assets/styled-logo.svg",
            svg,
            &AssetHash::sha256_bytes(svg),
        )
        .unwrap();

    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 80, 48).unwrap();
    backend.register_compiled_asset(&vector).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(8, 10, 14, 255)).unwrap();
    backend.draw_vector("styled-logo").unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();

    assert_eq!(
        snapshot.pixel_at(16, 16),
        Some(0x0000_cc66),
        "styled SVG fill color must survive asset lowering and Skia registration"
    );
}

#[test]
fn skia_backend_replays_runtime_scene_frame_commands() {
    let mut assets = AssetBackend::new(AssetLimits::default());
    let image_bytes = png_1x1();
    let image = assets
        .compile_image(
            "hero",
            "assets/hero.png",
            &image_bytes,
            &AssetHash::sha256_bytes(&image_bytes),
        )
        .unwrap();
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 48"><path d="M0 0 L32 0 L32 16 L0 16 Z"/></svg>"#;
    let vector = assets
        .compile_vector(
            "logo",
            "assets/logo.svg",
            svg,
            &AssetHash::sha256_bytes(svg),
        )
        .unwrap();
    let root_id = RuntimeViewId::new("root");
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        root_id.clone(),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(128.0, 96.0)),
        RuntimeVisual::Fill(Color::rgba(8, 10, 14, 255)),
    ))
    .with_child(
        &root_id,
        RuntimeViewNode::new(
            RuntimeViewId::new("image"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(24.0, 24.0)),
            RuntimeVisual::ImageAsset("hero".to_string()),
        ),
    )
    .unwrap()
    .with_child(
        &root_id,
        RuntimeViewNode::new(
            RuntimeViewId::new("vector"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(32.0, 16.0)),
            RuntimeVisual::VectorAsset("logo".to_string()),
        ),
    )
    .unwrap()
    .with_child(
        &root_id,
        RuntimeViewNode::new(
            RuntimeViewId::new("meter"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(72.0, 18.0)),
            RuntimeVisual::CustomSurface(
                RuntimeCustomSurfaceVisual::new(CustomSurfaceCategory::Meter)
                    .with_data_snapshot(CustomSurfaceDataSnapshot::new([0.2, 0.6, 1.0]).unwrap()),
            ),
        ),
    )
    .unwrap();
    let frame = RuntimeSceneBridge::new(Viewport::new(128.0, 96.0))
        .build(&tree)
        .unwrap();

    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 128, 96).unwrap();
    backend.register_compiled_asset(&image).unwrap();
    backend.register_compiled_asset(&vector).unwrap();
    backend.begin_frame("main").unwrap();
    backend.draw_runtime_scene_frame(&frame, 4, 1.0).unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();
    assert!(count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(0.0, 0.0, 128.0, 96.0)) > 0);
    assert!(backend.command_keys().iter().any(|key| {
        key.starts_with("runtime-scene-frame:commands=") && key.contains(":frame=4:dpi=1")
    }));
}

#[test]
fn vector_gradient_and_effects_render_pixels() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 128, 80).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(8, 10, 14, 255))
        .unwrap();
    backend
        .draw_filled_path(
            "M10 10 L30 10 L30 30 L10 30 Z",
            hawk2ui_render::Color::rgba(20, 220, 120, 255),
        )
        .unwrap();
    backend
        .draw_rounded_rect(
            Geometry::new(40.0, 10.0, 24.0, 20.0),
            6.0,
            hawk2ui_render::Color::rgba(80, 140, 255, 255),
        )
        .unwrap();
    backend
        .draw_linear_gradient(
            Geometry::new(72.0, 10.0, 42.0, 20.0),
            hawk2ui_render::Color::rgba(255, 64, 64, 255),
            hawk2ui_render::Color::rgba(64, 128, 255, 255),
        )
        .unwrap();
    backend
        .draw_shadow_rect(
            Geometry::new(12.0, 46.0, 20.0, 12.0),
            4.0,
            4.0,
            4.0,
            hawk2ui_render::Color::rgba(0, 0, 0, 180),
        )
        .unwrap();
    backend
        .draw_glow_rect(
            Geometry::new(54.0, 46.0, 20.0, 12.0),
            5.0,
            hawk2ui_render::Color::rgba(80, 220, 255, 180),
        )
        .unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();

    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(10.0, 10.0, 20.0, 20.0)) > 0,
        "filled vector path must affect its path bounds"
    );
    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(40.0, 10.0, 24.0, 20.0)) > 0,
        "rounded rectangle must affect its requested bounds"
    );
    assert_ne!(
        snapshot.pixel_at(74, 20),
        snapshot.pixel_at(112, 20),
        "linear gradient should produce different start and end colors"
    );
    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(16.0, 50.0, 26.0, 18.0)) > 0,
        "shadow must affect pixels in the offset blur region"
    );
    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(50.0, 42.0, 30.0, 22.0)) > 0,
        "glow must affect pixels around the source rectangle"
    );
}

#[test]
fn opacity_group_composites_children_at_group_alpha() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 64, 48).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(0, 0, 0, 255))
        .unwrap();
    backend.begin_opacity_group(0.5).unwrap();
    backend
        .fill(
            Geometry::new(8.0, 8.0, 32.0, 24.0),
            hawk2ui_render::Color::rgba(255, 0, 0, 255),
        )
        .unwrap();
    backend.end_opacity_group().unwrap();
    backend.end_frame("main").unwrap();

    let pixel = backend
        .frame_snapshot("main")
        .unwrap()
        .pixel_at(16, 16)
        .expect("inside opacity group pixel exists");
    let red = (pixel >> 16) & 0xff;
    assert!(
        (120..=136).contains(&red),
        "group alpha must blend child pixels; pixel={pixel:#08x}, red={red}"
    );
    assert_ne!(pixel, 0x00ff_0000, "child must not be drawn fully opaque");
}

#[test]
fn apply_layer_effect_executes_supported_structured_effects() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 128, 80).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(8, 10, 14, 255))
        .unwrap();
    backend
        .apply_layer_effect("shadow-rect:12,46,20,12:4,4:4:0,0,0,180")
        .unwrap();
    backend
        .apply_layer_effect("glow-rect:54,46,20,12:5:80,220,255,180")
        .unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();
    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(16.0, 50.0, 26.0, 18.0)) > 0,
        "structured shadow effect must render pixels"
    );
    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(50.0, 42.0, 30.0, 22.0)) > 0,
        "structured glow effect must render pixels"
    );
}

#[test]
fn runtime_shader_effect_draws_with_typed_uniforms_and_cache_stats() {
    let mut backend = SkiaRendererBackend::new();
    backend
        .register_runtime_shader_effect(
            "solid-orange",
            "uniform float4 color; half4 main(float2 p) { return half4(color); }",
        )
        .expect("runtime shader effect compiles");
    backend
        .register_runtime_shader_effect(
            "solid-orange",
            "uniform float4 color; half4 main(float2 p) { return half4(color); }",
        )
        .expect("duplicate effect registration is cache-stable");
    assert_eq!(backend.runtime_effect_cache_stats().compiled_effects(), 1);

    backend.create_surface("main", 64, 48).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(0, 0, 0, 255)).unwrap();
    backend
        .draw_runtime_effect_rect(
            "solid-orange",
            Geometry::new(8.0, 8.0, 24.0, 16.0),
            &[SkiaRuntimeEffectUniform::float4(
                "color",
                [1.0, 0.35, 0.0, 1.0],
            )],
            &[],
        )
        .expect("runtime shader effect draws with bound uniform");
    backend.end_frame("main").unwrap();

    let pixel = backend
        .frame_snapshot("main")
        .unwrap()
        .pixel_at(12, 12)
        .unwrap();
    assert!(
        ((pixel >> 16) & 0xff) > 220 && ((pixel >> 8) & 0xff) > 60,
        "runtime effect must paint using the supplied uniform color; pixel={pixel:#08x}"
    );
    assert_eq!(backend.runtime_effect_cache_stats().draw_calls(), 1);
    assert!(
        backend
            .command_keys()
            .contains(&"runtime-effect:solid-orange:uniforms=1:children=0".to_string())
    );
}

#[test]
fn runtime_scene_replay_renders_shader_effect_draw_commands() {
    let effect = RuntimeShaderEffectVisual::new(
        "solid-green",
        "uniform float4 color; half4 main(float2 p) { return half4(color); }",
    )
    .with_uniform(ShaderEffectUniform::float4("color", [0.0, 1.0, 0.0, 1.0]));
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(32.0, 24.0)),
        RuntimeVisual::ShaderEffect(effect),
    ));
    let frame = RuntimeSceneBridge::new(Viewport::new(48.0, 32.0))
        .build(&tree)
        .expect("shader effect scene builds");

    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 48, 32).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(0, 0, 0, 255)).unwrap();
    backend
        .draw_runtime_scene_frame(&frame, 5, 1.0)
        .expect("shader effect command replays through Skia");
    backend.end_frame("main").unwrap();

    assert_eq!(
        backend.frame_snapshot("main").unwrap().pixel_at(8, 8),
        Some(0x0000_ff00)
    );
    assert!(
        backend
            .command_keys()
            .contains(&"runtime-effect:solid-green:uniforms=1:children=0".to_string())
    );
}

#[test]
fn runtime_shader_effect_binds_registered_image_child_shader() {
    let mut backend = SkiaRendererBackend::new();
    backend.register_image_asset("red", &png_1x1()).unwrap();
    backend
        .register_runtime_shader_effect(
            "sample-image",
            "uniform shader image; half4 main(float2 p) { return image.eval(p); }",
        )
        .expect("runtime shader with image child compiles");

    backend.create_surface("main", 32, 32).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(0, 0, 0, 255)).unwrap();
    backend
        .draw_runtime_effect_rect(
            "sample-image",
            Geometry::new(0.0, 0.0, 24.0, 24.0),
            &[],
            &[SkiaRuntimeEffectChildInput::image("image", "red")],
        )
        .expect("runtime shader samples registered image child");
    backend.end_frame("main").unwrap();

    assert_eq!(
        backend.frame_snapshot("main").unwrap().pixel_at(8, 8),
        Some(0x00ff_0000)
    );
}

#[test]
fn runtime_shader_effect_rejects_invalid_source_uniforms_and_children() {
    let mut backend = SkiaRendererBackend::new();
    let compile_error = backend
        .register_runtime_shader_effect("bad-source", "not sksl")
        .expect_err("invalid SkSL source must be rejected at registration");
    assert_eq!(
        compile_error.diagnostic().rule(),
        "skia.runtime-effect.compile"
    );

    backend
        .register_runtime_shader_effect(
            "needs-bindings",
            "uniform float2 amount; uniform shader image; half4 main(float2 p) { return image.eval(p) * half4(amount.x, amount.y, 1.0, 1.0); }",
        )
        .expect("runtime shader compiles");
    backend.register_image_asset("red", &png_1x1()).unwrap();
    backend.create_surface("main", 32, 32).unwrap();
    backend.begin_frame("main").unwrap();

    let uniform_error = backend
        .draw_runtime_effect_rect(
            "needs-bindings",
            Geometry::new(0.0, 0.0, 16.0, 16.0),
            &[SkiaRuntimeEffectUniform::float("amount", 1.0)],
            &[SkiaRuntimeEffectChildInput::image("image", "red")],
        )
        .expect_err("wrong uniform arity must be rejected before drawing");
    assert_eq!(
        uniform_error.diagnostic().rule(),
        "skia.runtime-effect.uniform-invalid"
    );

    let child_error = backend
        .draw_runtime_effect_rect(
            "needs-bindings",
            Geometry::new(0.0, 0.0, 16.0, 16.0),
            &[SkiaRuntimeEffectUniform::float2("amount", [1.0, 1.0])],
            &[],
        )
        .expect_err("declared child shader must be bound explicitly");
    assert_eq!(
        child_error.diagnostic().rule(),
        "skia.runtime-effect.child-missing"
    );
}

#[test]
fn cache_lifecycle_tracks_generation_and_invalidation() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 96, 64).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(8, 10, 14, 255))
        .unwrap();
    backend
        .fill(
            Geometry::new(8.0, 8.0, 18.0, 18.0),
            hawk2ui_render::Color::rgba(240, 80, 60, 255),
        )
        .unwrap();
    backend
        .cache_current_frame_region("meter-cache", Geometry::new(8.0, 8.0, 18.0, 18.0))
        .unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(8, 10, 14, 255))
        .unwrap();
    backend
        .draw_cached_layer("meter-cache", Geometry::new(56.0, 8.0, 18.0, 18.0))
        .unwrap();

    let initial = backend.layer_cache("meter-cache").unwrap();
    assert_eq!(initial.generation(), 1);
    assert!(initial.valid());
    assert_eq!(initial.width(), 18);
    assert_eq!(initial.height(), 18);

    backend
        .mark_dirty(Geometry::new(10.0, 10.0, 2.0, 2.0))
        .unwrap();
    let invalidated = backend.layer_cache("meter-cache").unwrap();
    assert_eq!(invalidated.generation(), 2);
    assert!(!invalidated.valid());

    let error = backend
        .draw_cached_layer("meter-cache", Geometry::new(8.0, 36.0, 18.0, 18.0))
        .expect_err("invalidated cache must not be replayed");
    assert_eq!(error.diagnostic().rule(), "skia.cache.invalid");

    backend.end_frame("main").unwrap();
    let snapshot = backend.frame_snapshot("main").unwrap();
    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(56.0, 8.0, 18.0, 18.0)) > 0,
        "cache replay before invalidation must draw real cached pixels"
    );
}

#[test]
fn custom_meter_surface_renders_pixels_and_records_frame_limited_draw() {
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 128, 64).unwrap();
    backend.begin_frame("main").unwrap();
    backend.clear(Color::rgba(8, 10, 14, 255)).unwrap();

    let surface = CustomDrawSurface::new(
        "meter",
        CustomSurfaceCategory::Meter,
        Geometry::new(16.0, 20.0, 96.0, 24.0),
    )
    .with_frame_interval(2)
    .schedule_frame(4);
    let request = CustomSurfaceDrawRequest::new(
        surface,
        CustomSurfaceFrameContext::new(4, 1.0).unwrap(),
        CustomSurfaceDataSnapshot::new([0.1, 0.4, 0.8, 1.0]).unwrap(),
    )
    .unwrap();

    backend.draw_custom_surface(&request).unwrap();
    backend.end_frame("main").unwrap();

    let snapshot = backend.frame_snapshot("main").unwrap();
    assert!(
        count_changed_pixels(snapshot, 0x0008_0a0e, Geometry::new(16.0, 20.0, 96.0, 24.0)) > 0,
        "custom meter draw hook must produce visible pixels"
    );
    assert!(
        backend
            .command_keys()
            .iter()
            .any(|key| { key == "custom-surface:meter:meter:frame=4:samples=4" })
    );
}

#[test]
fn skia_backend_reports_structured_diagnostics_for_invalid_lifecycle() {
    let mut backend = SkiaRendererBackend::new();

    let draw_error = backend
        .draw_text("outside frame")
        .expect_err("drawing without a frame must fail");
    assert_eq!(draw_error.diagnostic().rule(), "skia.frame.missing");

    backend.create_surface("main", 320, 200).unwrap();
    let duplicate_error = backend
        .create_surface("main", 320, 200)
        .expect_err("duplicate surfaces must fail");
    assert_eq!(
        duplicate_error.diagnostic().rule(),
        "skia.surface.duplicate"
    );

    let resize_error = backend
        .resize_surface("main", 0, 200, 1.0)
        .expect_err("zero dimensions must fail");
    assert_eq!(
        resize_error.diagnostic().rule(),
        "skia.surface.invalid-size"
    );

    backend.begin_frame("main").unwrap();
    let fill_error = backend
        .fill(
            Geometry::new(0.0, 0.0, f32::NAN, 20.0),
            hawk2ui_render::Color::rgba(255, 255, 255, 255),
        )
        .expect_err("invalid fill geometry must fail");
    assert_eq!(fill_error.diagnostic().rule(), "skia.geometry.invalid");

    let transform_error = backend
        .push_transform(Transform::translate(f32::INFINITY, 0.0))
        .expect_err("invalid transforms must fail");
    assert_eq!(
        transform_error.diagnostic().rule(),
        "skia.transform.invalid"
    );
    backend.end_frame("main").unwrap();

    assert_eq!(backend.diagnostics().len(), 5);
}

fn drive_core_frame(backend: &mut impl RendererBackend) {
    backend.create_surface("main", 800, 600).unwrap();
    backend.resize_surface("main", 1024, 768, 2.0).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(0, 0, 0, 255))
        .unwrap();
    backend
        .fill(
            Geometry::new(0.0, 0.0, 100.0, 50.0),
            hawk2ui_render::Color::rgba(255, 0, 0, 255),
        )
        .unwrap();
    backend
        .stroke(
            Geometry::new(0.0, 0.0, 100.0, 50.0),
            hawk2ui_render::Stroke::new(2.0),
        )
        .unwrap();
    backend.draw_path("M0 0L10 10").unwrap();
    backend.draw_text("Hello").unwrap();
    backend.draw_image("hero").unwrap();
    backend
        .push_clip(Geometry::new(0.0, 0.0, 80.0, 40.0))
        .unwrap();
    backend
        .push_transform(Transform::translate(4.0, 8.0))
        .unwrap();
    backend
        .apply_layer_effect("shadow-rect:0,0,100,50:4,4:4:0,0,0,180")
        .unwrap();
    let cache = backend.create_cache_handle("card").unwrap();
    assert_eq!(cache.as_str(), "card");
    backend
        .mark_dirty(Geometry::new(0.0, 0.0, 100.0, 50.0))
        .unwrap();
    backend.end_frame("main").unwrap();
    backend.teardown_surface("main").unwrap();
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn count_changed_pixels(
    snapshot: &hawk2ui_render_skia::SkiaFrameSnapshot,
    background: u32,
    geometry: Geometry,
) -> usize {
    let x0 = geometry.x.max(0.0).floor() as u32;
    let y0 = geometry.y.max(0.0).floor() as u32;
    let x1 = (geometry.x + geometry.width)
        .ceil()
        .clamp(0.0, snapshot.width() as f32) as u32;
    let y1 = (geometry.y + geometry.height)
        .ceil()
        .clamp(0.0, snapshot.height() as f32) as u32;

    (y0..y1)
        .flat_map(|y| (x0..x1).filter_map(move |x| snapshot.pixel_at(x, y)))
        .filter(|pixel| *pixel != background)
        .count()
}

#[test]
fn skia_backend_reports_detailed_capabilities_and_vector_commands() {
    let mut backend = SkiaRendererBackend::new();

    let capabilities = backend.skia_capabilities();
    assert!(capabilities.cpu_raster.is_supported());
    assert!(capabilities.paths.is_supported());
    assert!(capabilities.clips.is_supported());
    assert!(capabilities.transforms.is_supported());
    assert!(capabilities.text.is_supported());
    assert!(capabilities.images.is_supported());
    assert!(capabilities.vectors.is_supported());
    assert!(capabilities.effects.is_supported());
    assert!(capabilities.runtime_effects.is_supported());
    assert!(capabilities.dirty_regions.is_supported());

    backend.create_surface("main", 320, 180).unwrap();
    backend
        .register_vector_paths("logo", ["M10 10 L30 10 L30 30 L10 30 Z"])
        .unwrap();
    backend.begin_frame("main").unwrap();
    backend.draw_vector("logo").unwrap();
    backend.end_frame("main").unwrap();

    assert!(backend.command_keys().contains(&"vector:logo".to_string()));
}

const ONE_BY_ONE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 15, 4, 0, 9, 251, 3,
    253, 167, 175, 213, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn png_1x1() -> Vec<u8> {
    png_rgba(&[255, 0, 0, 255], 1, 1)
}

fn png_rgba(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .write_image(pixels, width, height, ColorType::Rgba8.into())
        .expect("test PNG encodes");
    bytes
}
