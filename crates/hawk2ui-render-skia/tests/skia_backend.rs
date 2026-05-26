use hawk2ui_render::{BackendCapabilities, Geometry, RendererBackend, Transform};
use hawk2ui_render_skia::{SkiaRendererBackend, SkiaSurfaceConfig};

#[test]
fn skia_backend_matches_recording_backend_for_core_frame_commands() {
    let capabilities = BackendCapabilities::new()
        .with_gpu(false)
        .with_text(true)
        .with_images(true);
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
    assert_eq!(surface.dpi_scale(), 2.0);
    assert_eq!(surface.pixel_width(), 1600);
    assert_eq!(surface.pixel_height(), 1200);
    assert_eq!(surface.presented_frames(), 1);
    assert_eq!(surface.dirty_regions().len(), 1);
    assert!(!surface.frame_active());
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
    assert_eq!(snapshot.pixel_at(0, 0), Some(0x000000));
    assert_eq!(snapshot.pixel_at(3, 3), Some(0xff0000));
    assert_eq!(
        backend.surface("main").unwrap().presented_frames(),
        1,
        "ending a frame presents exactly one frame"
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
        count_changed_pixels(snapshot, 0x080a0e, Geometry::new(16.0, 22.0, 72.0, 28.0)) > 0,
        "placed text must affect pixels near its requested baseline"
    );
    assert!(
        count_changed_pixels(snapshot, 0x080a0e, Geometry::new(84.0, 24.0, 24.0, 24.0)) > 0,
        "target-rectangle image draw must affect pixels in the requested rectangle"
    );
    assert_eq!(
        count_changed_pixels(snapshot, 0x080a0e, Geometry::new(0.0, 0.0, 8.0, 8.0)),
        0,
        "unaffected background corner should remain unchanged"
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

    assert_eq!(backend.diagnostics().len(), 3);
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
    backend.apply_layer_effect("shadow").unwrap();
    let cache = backend.create_cache_handle("card").unwrap();
    assert_eq!(cache.as_str(), "card");
    backend
        .mark_dirty(Geometry::new(0.0, 0.0, 100.0, 50.0))
        .unwrap();
    backend.end_frame("main").unwrap();
    backend.teardown_surface("main").unwrap();
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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
    assert!(capabilities.dirty_regions.is_supported());

    backend.create_surface("main", 320, 180).unwrap();
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
