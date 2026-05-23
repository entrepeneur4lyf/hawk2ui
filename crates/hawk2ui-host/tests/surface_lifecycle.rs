use hawk2ui_host::{
    FramePresenter, HostCapabilities, HostSurface, RecordingFramePresenter, RecordingHostSurface,
    RepaintRequest, SurfaceEvent, SurfaceMetrics,
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
