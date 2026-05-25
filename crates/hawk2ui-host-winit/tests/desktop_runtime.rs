use hawk2ui_host::{DesktopWindowConfig, SurfaceMetrics};
use hawk2ui_host_winit::{DesktopRuntimeEvent, SoftwareFrameRenderer, WinitDesktopRuntimeConfig};

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
fn runtime_config_rejects_zero_size() {
    let config = WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
        "app",
        SurfaceMetrics::new(0.0, 480.0, 1.0),
    ));

    let error = config.validate().expect_err("zero width must fail");

    assert_eq!(error.rule(), "desktop.window.invalid-size");
}

#[test]
fn runtime_config_accepts_first_frame_smoke_mode() {
    let config = WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
        "app",
        SurfaceMetrics::new(640.0, 480.0, 1.0),
    ))
    .with_exit_after_first_frame(true);

    config.validate().expect("valid runtime config");
    assert!(config.exit_after_first_frame());
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
