use hawk2ui_api::{
    BindingDirection, CapabilityKey, FrameSchedule, HostBindingContract, HostSurfaceContract,
    InputEvent, KeyEvent, KeyModifiers, MouseButton, RepaintReason, RepaintRequest, RuntimeJob,
    RuntimeJobId, RuntimeJobKind, RuntimeJobStatus, RuntimeLifecycleHook, RuntimePhase,
    SurfaceKind, SurfaceMetrics,
};

#[test]
fn surface_runtime_contracts_downstream_code_uses_surface_events_and_repaint_contracts_from_root_exports()
 {
    let metrics = SurfaceMetrics::new(1280.0, 720.0, 2560, 1440, 2.0);
    let surface = HostSurfaceContract::new(SurfaceKind::Desktop, metrics, true);
    let event = InputEvent::PointerPressed {
        button: MouseButton::Primary,
        x: 42.0,
        y: 24.0,
    };
    let repaint = RepaintRequest::new(RepaintReason::Input, 9001, FrameSchedule::Immediate);

    assert_eq!(surface.kind, SurfaceKind::Desktop);
    assert_eq!(surface.metrics.physical_width, 2560);
    assert_eq!(event.surface_position(), Some((42.0, 24.0)));
    assert!(repaint.is_immediate());
}

#[test]
fn surface_metrics_sanitize_non_finite_or_invalid_values() {
    let metrics = SurfaceMetrics::new(f32::NAN, f32::INFINITY, 0, 0, -1.0);

    assert!((metrics.logical_width - 0.0).abs() < f32::EPSILON);
    assert!((metrics.logical_height - 0.0).abs() < f32::EPSILON);
    assert!((metrics.scale_factor - 1.0).abs() < f32::EPSILON);
}

#[test]
fn surface_runtime_contracts_downstream_code_uses_keyboard_and_lifecycle_contracts_from_root_exports()
 {
    let event = InputEvent::KeyPressed(KeyEvent::new(
        "Enter",
        Some("NumpadEnter"),
        KeyModifiers::empty().with_shift().with_meta(),
        false,
    ));
    let hook = RuntimeLifecycleHook::new(RuntimePhase::Running, "mount_ui");

    assert!(event.requires_focus());
    assert_eq!(hook.phase, RuntimePhase::Running);
    assert_eq!(hook.name, "mount_ui");
}

#[test]
fn surface_runtime_contracts_downstream_code_uses_runtime_jobs_and_binding_records_from_root_exports()
 {
    let binding = HostBindingContract::new(
        "clipboard.write_text",
        CapabilityKey::new("clipboard"),
        RuntimePhase::Running,
    )
    .with_direction(BindingDirection::RuntimeToHost);

    let job = RuntimeJob::new(
        RuntimeJobId::new("job-load-main"),
        RuntimeJobKind::InvokeHostBinding,
        RuntimePhase::Running,
    )
    .with_capability(CapabilityKey::new("clipboard"))
    .with_status(RuntimeJobStatus::Running);

    assert_eq!(binding.direction, BindingDirection::RuntimeToHost);
    assert_eq!(job.id.as_str(), "job-load-main");
    assert_eq!(
        job.required_capability.as_ref().unwrap().as_str(),
        "clipboard"
    );
    assert_eq!(job.status, RuntimeJobStatus::Running);
}

#[test]
fn surface_runtime_contracts_surface_and_runtime_records_are_stable_json_contracts() {
    let event = InputEvent::FocusChanged(true);
    let job = RuntimeJob::new(
        RuntimeJobId::new("job-render-frame"),
        RuntimeJobKind::RenderFrame,
        RuntimePhase::Running,
    );

    assert_eq!(
        serde_json::to_value(&event).expect("event json"),
        serde_json::json!({"FocusChanged": true})
    );
    assert_eq!(
        serde_json::to_value(&job).expect("job json"),
        serde_json::json!({
            "id": "job-render-frame",
            "kind": "RenderFrame",
            "phase": "Running",
            "status": "Pending",
            "required_capability": null
        })
    );
}
