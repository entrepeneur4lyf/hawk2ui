use std::collections::BTreeMap;

use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use hawk2ui_layout::{
    BoxEdges, FlexDirection, LayoutSizing, LayoutStyle, LayoutValue, TestTextMeasurer, Viewport,
};
use hawk2ui_render::{
    Color, CustomSurfaceCategory, CustomSurfaceDataSnapshot, Geometry, RendererBackend,
    SceneNodeId, ShaderEffectChildInput, ShaderEffectUniform,
};
use hawk2ui_render_skia::{SkiaFrameSnapshot, SkiaRendererBackend};
use hawk2ui_runtime::{
    BindingExecution, BindingLifecycleAvailability, BindingSchema, HostBindingRecord,
    HostBindingRegistry, HostCallRecord, LifecycleHook, LifecyclePhase, LifecycleRegistry,
    PromiseId, RecordingScriptEngine, RuntimeCapability, RuntimeDrawCommand, RuntimeError,
    RuntimeEvent, RuntimeEventDispatcher, RuntimeEventKind, RuntimeEventPayload,
    RuntimeEventPropagation, RuntimeExecutionContext, RuntimeGuardOperation,
    RuntimePersistenceStore, RuntimeSafetyGuard, RuntimeSceneBridge, RuntimeSceneError,
    RuntimeSceneFrame, RuntimeScheduler, RuntimeShaderEffectVisual, RuntimeStateEntry,
    RuntimeStateMigration, RuntimeStateScope, RuntimeStateSnapshot, RuntimeStoragePath,
    RuntimeTextVisual, RuntimeViewId, RuntimeViewNode, RuntimeViewTree, RuntimeVisual,
    ScriptEngine, ScriptEngineOperation, ScriptModuleKind, ScriptModuleRecord, StructuredValue,
    TimerJob,
};
use serde::{Serialize, de::DeserializeOwned};

fn assert_serde_contract<T: Serialize + DeserializeOwned>() {}

#[test]
fn host_binding_error_converts_to_shared_diagnostic_with_context() {
    let registry = HostBindingRegistry::new([HostBindingRecord::new(
        "clipboard.write",
        BindingSchema::new("string", "null", "error"),
    )
    .requires(RuntimeCapability::ClipboardWrite)]);
    let error = registry
        .call(
            "clipboard.write",
            StructuredValue::String("copy".to_string()),
            [],
            LifecyclePhase::Mount,
        )
        .expect_err("capability denial is reported");
    let diagnostic = Diagnostic::from(error);

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "binding.capability-denied");
    assert!(
        diagnostic
            .related
            .iter()
            .any(|context| context.label == "binding" && context.value == "clipboard.write")
    );
}

fn runtime_scene_tree(invalidate_meter: bool) -> RuntimeViewTree {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(100.0, 100.0)),
        RuntimeVisual::Fill(Color::rgba(0, 0, 0, 255)),
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("meter"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(80.0, 20.0)),
            RuntimeVisual::Fill(Color::rgba(0, 200, 120, 255)),
        ),
    )
    .expect("meter attaches");
    if invalidate_meter {
        tree.invalidate(&RuntimeViewId::new("meter"))
            .expect("meter invalidates")
    } else {
        tree
    }
}

#[test]
fn runtime_records_module_identity_is_stable() {
    let module = ScriptModuleRecord::new("main", "app://main.js", ScriptModuleKind::JavaScript)
        .with_hash("sha256:abc")
        .requires(RuntimeCapability::RenderInvalidation)
        .exports("mount");

    assert_eq!(module.identity(), "main@app://main.js");
    assert_eq!(module.hash.as_deref(), Some("sha256:abc"));
    assert_eq!(module.exports, vec!["mount"]);
    assert_eq!(
        module.required_capabilities,
        vec![RuntimeCapability::RenderInvalidation]
    );
}

#[test]
fn runtime_records_report_host_call_errors() {
    let call = HostCallRecord::new(
        "main",
        "platform.clipboard.write",
        StructuredValue::object([("text", StructuredValue::string("hello"))]),
    )
    .requires(RuntimeCapability::ClipboardWrite);

    let error = RuntimeError::host_call_denied(
        call.binding_name.clone(),
        RuntimeCapability::ClipboardWrite,
        "clipboard.write is not declared",
    );

    assert_eq!(call.module_id, "main");
    assert_eq!(
        call.required_capability,
        Some(RuntimeCapability::ClipboardWrite)
    );
    assert_eq!(error.code, "runtime.host-call-denied");
    assert_eq!(error.capability, Some(RuntimeCapability::ClipboardWrite));
    assert!(error.message.contains("clipboard.write"));
}

#[test]
fn runtime_records_register_lifecycle_hooks_in_order() {
    let registry = LifecycleRegistry::new([
        LifecycleHook::new("main", LifecyclePhase::Initialize, "initialize"),
        LifecycleHook::new("main", LifecyclePhase::Mount, "mount"),
        LifecycleHook::new("main", LifecyclePhase::Update, "update"),
        LifecycleHook::new("main", LifecyclePhase::Suspend, "suspend"),
        LifecycleHook::new("main", LifecyclePhase::Resume, "resume"),
        LifecycleHook::new("main", LifecyclePhase::HotReload, "hot_reload"),
        LifecycleHook::new("main", LifecyclePhase::ErrorBoundary, "handle_error"),
        LifecycleHook::new("main", LifecyclePhase::Shutdown, "shutdown"),
        LifecycleHook::new("main", LifecyclePhase::Teardown, "dispose"),
    ]);

    let hook_names: Vec<_> = registry
        .hooks_for(LifecyclePhase::Mount)
        .iter()
        .map(|hook| hook.export_name.as_str())
        .collect();

    assert_eq!(hook_names, vec!["mount"]);
    assert_eq!(
        registry.hooks_for(LifecyclePhase::Teardown)[0].module_id,
        "main"
    );
    assert_eq!(
        registry
            .hooks_for(LifecyclePhase::HotReload)
            .iter()
            .map(|hook| hook.export_name.as_str())
            .collect::<Vec<_>>(),
        vec!["hot_reload"]
    );
    assert_eq!(registry.all().len(), 9);
}

#[test]
fn runtime_records_are_serializable_contracts() {
    assert_serde_contract::<ScriptModuleRecord>();
    assert_serde_contract::<HostCallRecord>();
    assert_serde_contract::<StructuredValue>();
    assert_serde_contract::<RuntimeError>();
    assert_serde_contract::<LifecycleHook>();
    assert_serde_contract::<LifecycleRegistry>();
}

#[test]
fn host_bindings_allow_capability_scoped_calls() {
    let registry = HostBindingRegistry::new([HostBindingRecord::new(
        "platform.clipboard.write",
        BindingSchema::new("object", "null", "ClipboardError"),
    )
    .requires(RuntimeCapability::ClipboardWrite)
    .execution(BindingExecution::Synchronous)
    .available_during(BindingLifecycleAvailability::AfterMount)]);

    let call = registry
        .call(
            "platform.clipboard.write",
            StructuredValue::object([("text", StructuredValue::string("hello"))]),
            [RuntimeCapability::ClipboardWrite],
            LifecyclePhase::Update,
        )
        .expect("declared capability should allow the host call");

    assert_eq!(call.binding_name, "platform.clipboard.write");
    assert_eq!(
        call.required_capability,
        Some(RuntimeCapability::ClipboardWrite)
    );
    assert_eq!(call.output_schema, "null");
}

#[test]
fn host_bindings_deny_missing_capability() {
    let registry = HostBindingRegistry::new([HostBindingRecord::new(
        "platform.network.fetch",
        BindingSchema::new("object", "object", "NetworkError"),
    )
    .requires(RuntimeCapability::NetworkRequest)]);

    let error = registry
        .call(
            "platform.network.fetch",
            StructuredValue::object([("url", StructuredValue::string("https://example.test"))]),
            [],
            LifecyclePhase::Update,
        )
        .expect_err("missing capability should deny the host call");

    assert_eq!(error.code, "binding.capability-denied");
    assert_eq!(error.capability, Some(RuntimeCapability::NetworkRequest));
}

#[test]
fn host_bindings_enforce_every_platform_runtime_capability_domain() {
    let domains = [
        (
            "platform.filesystem.read",
            RuntimeCapability::FilesystemAccess,
        ),
        ("platform.network.fetch", RuntimeCapability::NetworkRequest),
        ("platform.clipboard.read", RuntimeCapability::ClipboardRead),
        (
            "platform.clipboard.write",
            RuntimeCapability::ClipboardWrite,
        ),
        ("platform.database.query", RuntimeCapability::DatabaseAccess),
        ("platform.audio.playback", RuntimeCapability::AudioPlayback),
        ("platform.secrets.read", RuntimeCapability::SecretRead),
        ("platform.ai.request", RuntimeCapability::AiProvider),
        ("platform.mcp.call", RuntimeCapability::Mcp),
        ("platform.dialog.open", RuntimeCapability::Dialogs),
        (
            "platform.notification.send",
            RuntimeCapability::Notifications,
        ),
        (
            "platform.shortcut.register",
            RuntimeCapability::GlobalShortcuts,
        ),
    ];

    for (binding_name, capability) in domains {
        let registry = HostBindingRegistry::new([HostBindingRecord::new(
            binding_name,
            BindingSchema::new("object", "object", "PlatformError"),
        )
        .requires(capability)]);

        let denied = registry
            .call(
                binding_name,
                StructuredValue::Object(BTreeMap::default()),
                [],
                LifecyclePhase::Update,
            )
            .expect_err("missing declared capability must deny platform host binding");
        assert_eq!(denied.code, "binding.capability-denied");
        assert_eq!(denied.capability, Some(capability));

        let allowed = registry
            .call(
                binding_name,
                StructuredValue::Object(BTreeMap::default()),
                [capability],
                LifecyclePhase::Update,
            )
            .expect("declared capability must allow platform host binding");
        assert_eq!(allowed.required_capability, Some(capability));
    }
}

#[test]
fn host_bindings_preserve_first_declaration_for_duplicate_names() {
    let registry = HostBindingRegistry::new([
        HostBindingRecord::new(
            "platform.clipboard.write",
            BindingSchema::new("object", "null", "ClipboardError"),
        )
        .requires(RuntimeCapability::ClipboardWrite),
        HostBindingRecord::new(
            "platform.clipboard.write",
            BindingSchema::new("object", "null", "ClipboardError"),
        ),
    ]);

    let error = registry
        .call(
            "platform.clipboard.write",
            StructuredValue::object([("text", StructuredValue::string("hello"))]),
            [],
            LifecyclePhase::Update,
        )
        .expect_err("duplicate binding must not downgrade the protected declaration");

    assert_eq!(error.code, "binding.capability-denied");
    assert_eq!(error.capability, Some(RuntimeCapability::ClipboardWrite));
}

#[test]
fn host_bindings_reject_schema_mismatch() {
    let registry = HostBindingRegistry::new([HostBindingRecord::new(
        "platform.clipboard.write",
        BindingSchema::new("object", "null", "ClipboardError"),
    )]);

    let error = registry
        .call(
            "platform.clipboard.write",
            StructuredValue::string("not an object"),
            [],
            LifecyclePhase::Update,
        )
        .expect_err("string payload should not satisfy object schema");

    assert_eq!(error.code, "binding.schema-mismatch");
    assert!(error.message.contains("expected object"));
}

#[test]
fn host_bindings_reject_unavailable_lifecycle_phase() {
    let registry = HostBindingRegistry::new([HostBindingRecord::new(
        "host.window.close",
        BindingSchema::new("null", "null", "WindowError"),
    )
    .available_during(BindingLifecycleAvailability::MountedOnly)]);

    let error = registry
        .call(
            "host.window.close",
            StructuredValue::Null,
            [],
            LifecyclePhase::Initialize,
        )
        .expect_err("mounted-only binding should not run during initialize");

    assert_eq!(error.code, "binding.lifecycle-unavailable");
}

#[test]
fn event_dispatch_preserves_enqueue_order_across_event_kinds() {
    let mut dispatcher = RuntimeEventDispatcher::default();
    dispatcher.listen("root", RuntimeEventKind::Ui);
    dispatcher.listen("app", RuntimeEventKind::Lifecycle);
    dispatcher.listen("meter", RuntimeEventKind::PluginParameter);
    dispatcher.listen("host", RuntimeEventKind::HostCallback);

    dispatcher.enqueue(RuntimeEvent::ui("root", "click"));
    dispatcher.enqueue(RuntimeEvent::lifecycle("app", "lifecycle.hot-reloaded"));
    dispatcher.enqueue(RuntimeEvent::plugin_parameter("meter", "gain", 0.75));
    dispatcher.enqueue(RuntimeEvent::host_callback("host", "window.close"));

    let deliveries = dispatcher
        .dispatch_pending()
        .expect("dispatch should succeed");
    let names: Vec<_> = deliveries
        .iter()
        .map(|delivery| delivery.event.name.as_str())
        .collect();

    assert_eq!(
        names,
        vec!["click", "lifecycle.hot-reloaded", "gain", "window.close"]
    );
}

#[test]
fn event_dispatch_bubbles_from_target_to_ancestors() {
    let mut dispatcher = RuntimeEventDispatcher::default();
    dispatcher.listen("button", RuntimeEventKind::Custom);
    dispatcher.listen("panel", RuntimeEventKind::Custom);
    dispatcher.listen("root", RuntimeEventKind::Custom);

    dispatcher.enqueue(
        RuntimeEvent::custom("button", "armed", RuntimeEventPayload::Null)
            .with_bubble_path(["panel", "root"])
            .propagation(RuntimeEventPropagation::Bubble),
    );

    let deliveries = dispatcher
        .dispatch_pending()
        .expect("dispatch should succeed");
    let targets: Vec<_> = deliveries
        .iter()
        .map(|delivery| delivery.listener_target.as_str())
        .collect();

    assert_eq!(targets, vec!["button", "panel", "root"]);
}

#[test]
fn event_dispatch_cancels_pending_events_after_teardown() {
    let mut dispatcher = RuntimeEventDispatcher::default();
    dispatcher.listen("root", RuntimeEventKind::Ui);
    dispatcher.enqueue(RuntimeEvent::ui("root", "click"));

    dispatcher.begin_teardown();
    let error = dispatcher
        .dispatch_pending()
        .expect_err("teardown should cancel pending dispatch");

    assert_eq!(error.code, "event.teardown-cancelled");
    assert!(dispatcher.is_empty());
}

#[test]
fn event_delivery_applies_visual_update_and_repaints_rendered_output() {
    let lifecycle = LifecycleRegistry::new([
        LifecycleHook::new("main", LifecyclePhase::Mount, "mount"),
        LifecycleHook::new("main", LifecyclePhase::Update, "paintPressed"),
        LifecycleHook::new("main", LifecyclePhase::Teardown, "dispose"),
    ]);
    let mut dispatcher = RuntimeEventDispatcher::default();
    dispatcher.listen("button", RuntimeEventKind::Ui);
    dispatcher.enqueue(RuntimeEvent::ui("button", "press"));

    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(120.0, 80.0)),
        RuntimeVisual::Fill(Color::rgba(8, 10, 14, 255)),
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("button"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(80.0, 32.0)),
            RuntimeVisual::Fill(Color::rgba(30, 30, 30, 255)),
        ),
    )
    .expect("button attaches");
    let bridge = RuntimeSceneBridge::new(Viewport::new(120.0, 80.0));
    let before = bridge.build(&tree).expect("initial frame builds");

    let deliveries = dispatcher
        .dispatch_pending()
        .expect("event dispatch should deliver before teardown");
    assert_eq!(deliveries[0].listener_target, "button");
    assert_eq!(
        lifecycle.hooks_for(LifecyclePhase::Update)[0].export_name,
        "paintPressed"
    );

    let updated = tree
        .update_visual(
            &RuntimeViewId::new("button"),
            RuntimeVisual::Fill(Color::rgba(240, 88, 40, 255)),
        )
        .expect("event handler should update the visual");
    let after = bridge.build(&updated).expect("updated frame builds");
    let diff = after.diff_from(&before).expect("frame diff builds");

    assert!(diff.requires_repaint());
    assert_eq!(after.invalidated_view_ids(), [RuntimeViewId::new("button")]);
    assert!(after.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::Fill { id, color, .. }
                if id.as_str() == "button" && *color == Color::rgba(240, 88, 40, 255)
        )
    }));
}

#[test]
fn scheduler_batches_runtime_work_in_priority_order() {
    let mut scheduler = RuntimeScheduler::default();
    scheduler.schedule_script_job("hydrate");
    scheduler.schedule_host_callback("host.window.open");
    scheduler.schedule_ui_event(RuntimeEvent::ui("root", "click"));
    scheduler.invalidate_render("root");
    scheduler.schedule_animation_tick(16);
    scheduler.schedule_timer(TimerJob::new("debounce", 32));

    let batch = scheduler.drain_batch().expect("scheduler should drain");

    assert_eq!(batch.script_jobs, vec!["hydrate"]);
    assert_eq!(batch.host_callbacks, vec!["host.window.open"]);
    assert_eq!(batch.ui_events[0].name, "click");
    assert_eq!(batch.render_invalidations, vec!["root"]);
    assert_eq!(batch.animation_ticks, vec![16]);
    assert_eq!(batch.timers[0].id, "debounce");
}

#[test]
fn scheduler_coalesces_render_invalidations() {
    let mut scheduler = RuntimeScheduler::default();
    scheduler.invalidate_render("root");
    scheduler.invalidate_render("meter");
    scheduler.invalidate_render("root");

    let batch = scheduler.drain_batch().expect("scheduler should drain");

    assert_eq!(batch.render_invalidations, vec!["meter", "root"]);
}

#[test]
fn scheduler_consumes_scene_update_for_repaint_and_cache_eviction() {
    let bridge = RuntimeSceneBridge::new(Viewport::new(100.0, 100.0));
    let previous = bridge
        .build(&runtime_scene_tree(false))
        .expect("previous frame builds");
    let next = bridge
        .build(&runtime_scene_tree(true))
        .expect("next frame builds");

    let update = next
        .diff_from(&previous)
        .expect("scene update diff succeeds");

    assert!(update.requires_repaint());
    assert_eq!(
        update.repaint_bounds(),
        Some(Geometry::new(0.0, 0.0, 80.0, 20.0))
    );
    assert_eq!(
        update.cache_invalidated_view_ids(),
        &[RuntimeViewId::new("meter"), RuntimeViewId::new("root")]
    );

    let mut scheduler = RuntimeScheduler::default();
    scheduler.schedule_scene_update(&update);
    let batch = scheduler.drain_batch().expect("scheduler should drain");

    assert_eq!(batch.render_invalidations, vec!["meter", "root"]);
    assert_eq!(batch.host_callbacks, vec!["host.repaint.scene-dirty"]);
}

#[test]
fn animation_frame_scheduler_produces_deterministic_primary_and_reduced_rate_ticks() {
    let policy = hawk2ui_runtime::AnimationCadencePolicy::new(60)
        .expect("60hz policy is valid")
        .with_reduced_rate_divisor(4)
        .expect("reduced-rate divisor is valid");
    let mut scheduler = hawk2ui_runtime::AnimationFrameScheduler::new(policy);

    assert_eq!(
        scheduler.step_at(0),
        Some(hawk2ui_runtime::AnimationFrameTick::new(0, 0, true))
    );
    assert_eq!(scheduler.step_at(15), None);
    assert_eq!(
        scheduler.step_at(17),
        Some(hawk2ui_runtime::AnimationFrameTick::new(1, 17, false))
    );
    assert_eq!(
        scheduler.step_at(68),
        Some(hawk2ui_runtime::AnimationFrameTick::new(2, 68, true))
    );
}

#[test]
fn animation_frame_scheduler_honors_reduced_motion_without_losing_forced_steps() {
    let policy = hawk2ui_runtime::AnimationCadencePolicy::new(60)
        .expect("60hz policy is valid")
        .with_reduced_motion(true);
    let mut scheduler = hawk2ui_runtime::AnimationFrameScheduler::new(policy);

    assert_eq!(scheduler.step_at(1_000), None);
    assert_eq!(
        scheduler.force_step(1_000),
        hawk2ui_runtime::AnimationFrameTick::new(0, 1_000, true)
    );
    assert_eq!(scheduler.step_at(1_017), None);
}

#[test]
fn scene_update_evicts_backend_caches_before_frame_replay() {
    let bridge = RuntimeSceneBridge::new(Viewport::new(100.0, 100.0));
    let previous = bridge
        .build(&runtime_scene_tree(false))
        .expect("previous frame builds");
    let next = bridge
        .build(&runtime_scene_tree(true))
        .expect("next frame builds");
    let update = next
        .diff_from(&previous)
        .expect("scene update diff succeeds");
    let mut backend = SkiaRendererBackend::new();
    backend.create_surface("main", 100, 100).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(Color::rgba(8, 10, 14, 255))
        .expect("surface clears");
    backend
        .fill(
            Geometry::new(0.0, 0.0, 100.0, 100.0),
            Color::rgba(16, 20, 28, 255),
        )
        .expect("root pixels draw");
    backend
        .cache_current_frame_region("root", Geometry::new(0.0, 0.0, 100.0, 100.0))
        .expect("root cache captures");
    backend
        .fill(
            Geometry::new(0.0, 0.0, 80.0, 20.0),
            Color::rgba(0, 200, 120, 255),
        )
        .expect("meter pixels draw");
    backend
        .cache_current_frame_region("meter", Geometry::new(0.0, 0.0, 80.0, 20.0))
        .expect("meter cache captures");
    backend.end_frame("main").unwrap();

    update
        .evict_backend_caches(&mut backend)
        .expect("cache evictions apply");

    assert!(!backend.layer_cache("root").unwrap().valid());
    assert!(!backend.layer_cache("meter").unwrap().valid());
    assert_eq!(
        backend.cache_invalidation_keys(),
        &[String::from("meter"), String::from("root")]
    );
}

#[test]
fn scheduler_cancels_pending_work_during_shutdown() {
    let mut scheduler = RuntimeScheduler::default();
    scheduler.schedule_script_job("hydrate");
    scheduler.invalidate_render("root");

    scheduler.begin_shutdown();
    let error = scheduler
        .drain_batch()
        .expect_err("shutdown should cancel pending work");

    assert_eq!(error.code, "scheduler.shutdown-cancelled");
    assert!(scheduler.is_empty());
}

#[test]
fn script_adapter_records_module_calls_promises_timers_and_host_calls() {
    let mut engine = RecordingScriptEngine::default();
    let module = ScriptModuleRecord::new(
        "main",
        "artifact://main.js",
        ScriptModuleKind::TypeScriptOutput,
    );
    let host_call = HostCallRecord::new(
        "main",
        "platform.clipboard.write",
        StructuredValue::object([("text", StructuredValue::string("hello"))]),
    );

    engine.load_module(module.clone()).expect("module loads");
    engine
        .call_export("main", "mount", StructuredValue::Null)
        .expect("export call records");
    engine
        .resolve_promise(PromiseId::new("promise-1"), StructuredValue::string("ok"))
        .expect("promise resolution records");
    engine
        .set_timer(TimerJob::new("timeout-1", 50))
        .expect("timer records");
    engine
        .call_host(host_call.clone())
        .expect("host call records");

    assert_eq!(
        engine.operations(),
        &[
            ScriptEngineOperation::LoadModule(module),
            ScriptEngineOperation::CallExport {
                module_id: "main".into(),
                export_name: "mount".into(),
                argument: StructuredValue::Null,
            },
            ScriptEngineOperation::ResolvePromise {
                promise_id: PromiseId::new("promise-1"),
                value: StructuredValue::string("ok"),
            },
            ScriptEngineOperation::SetTimer(TimerJob::new("timeout-1", 50)),
            ScriptEngineOperation::HostCall(host_call),
        ]
    );
}

#[test]
fn runtime_state_persistence_saves_restores_and_migrates_scoped_state() {
    let snapshot = RuntimeStateSnapshot::new(1)
        .with_entry(RuntimeStateEntry::new(
            RuntimeStateScope::App,
            "theme",
            StructuredValue::string("graphite"),
        ))
        .with_entry(RuntimeStateEntry::new(
            RuntimeStateScope::UiPreferences,
            "window.width",
            StructuredValue::Number(1280.0),
        ))
        .with_entry(RuntimeStateEntry::new(
            RuntimeStateScope::PluginParameter,
            "gain",
            StructuredValue::Number(0.5),
        ))
        .with_entry(RuntimeStateEntry::new(
            RuntimeStateScope::PluginNonParameter,
            "oversampling",
            StructuredValue::Bool(true),
        ))
        .with_entry(RuntimeStateEntry::new(
            RuntimeStateScope::UserPreset,
            "preset.user.wide",
            StructuredValue::string("wide"),
        ))
        .with_host_chunk("vst3", [1, 2, 3]);
    let migrated = snapshot
        .migrate([RuntimeStateMigration::rename_key(
            1,
            2,
            RuntimeStateScope::PluginParameter,
            "gain",
            "input.gain",
        )])
        .expect("migration should apply");

    let storage_path = RuntimeStoragePath::user_data("/home/user/.local/share/hawk2ui")
        .expect("storage path should be valid");
    let mut store = RuntimePersistenceStore::new(storage_path);
    store
        .save("plugin.delay", migrated.clone())
        .expect("snapshot should save");
    let restored = store
        .restore("plugin.delay")
        .expect("state should restore after restart");

    assert_eq!(restored.schema_version, 2);
    assert_eq!(
        restored.entry(RuntimeStateScope::PluginParameter, "input.gain"),
        Some(&StructuredValue::Number(0.5))
    );
    assert_eq!(
        restored.entry(RuntimeStateScope::UiPreferences, "window.width"),
        Some(&StructuredValue::Number(1280.0))
    );
    assert_eq!(restored.host_chunks()[0].format, "vst3");
    assert!(
        restored
            .entry(RuntimeStateScope::PluginParameter, "gain")
            .is_none()
    );
    assert!(store.user_preset_path("plugin.delay", "wide").is_ok());
}

#[test]
fn runtime_state_persistence_rejects_unsafe_paths_and_bad_migrations() {
    assert!(RuntimeStoragePath::user_data("relative/path").is_err());
    assert!(RuntimeStoragePath::user_data("/home/user/../secrets").is_err());
    assert!(RuntimeStoragePath::user_data("/home/user/./state").is_err());

    let snapshot = RuntimeStateSnapshot::new(3);
    let error = snapshot
        .migrate([RuntimeStateMigration::rename_key(
            1,
            2,
            RuntimeStateScope::App,
            "old",
            "new",
        )])
        .expect_err("migration source version mismatch must fail");

    assert_eq!(error.code, "state.migration-version-mismatch");
}

#[test]
fn script_adapter_interrupt_and_teardown_block_future_work() {
    let mut engine = RecordingScriptEngine::default();
    engine
        .interrupt("deadline exceeded")
        .expect("interrupt records");
    engine.teardown().expect("teardown records");

    let error = engine
        .call_export("main", "update", StructuredValue::Null)
        .expect_err("teardown should reject future script work");

    assert_eq!(error.code, "script-engine.teardown-complete");
    assert_eq!(
        engine.operations().last(),
        Some(&ScriptEngineOperation::Teardown)
    );
}

#[test]
fn plugin_safety_guard_denies_audio_thread_runtime_operations() {
    let guard = RuntimeSafetyGuard::for_context(RuntimeExecutionContext::AudioThread);

    for operation in [
        RuntimeGuardOperation::ScriptExecution,
        RuntimeGuardOperation::Rendering,
        RuntimeGuardOperation::Filesystem,
        RuntimeGuardOperation::Network,
        RuntimeGuardOperation::BlockingSynchronization,
    ] {
        let denial = guard
            .ensure_allowed(operation)
            .expect_err("audio-thread runtime operation should be denied");

        assert_eq!(denial.code, "runtime.audio-thread-operation-denied");
        assert_eq!(denial.context, RuntimeExecutionContext::AudioThread);
        assert_eq!(denial.operation, operation);
    }
}

#[test]
fn plugin_safety_guard_allows_realtime_safe_operations() {
    let guard = RuntimeSafetyGuard::for_context(RuntimeExecutionContext::AudioThread);

    guard
        .ensure_allowed(RuntimeGuardOperation::ParameterAutomation)
        .expect("parameter automation should be realtime-safe");
    guard
        .ensure_allowed(RuntimeGuardOperation::RealtimeDataWrite)
        .expect("lock-free realtime data writes should be allowed");
}

#[test]
fn plugin_safety_guard_allows_ui_thread_runtime_operations() {
    let guard = RuntimeSafetyGuard::for_context(RuntimeExecutionContext::UiThread);

    guard
        .ensure_allowed(RuntimeGuardOperation::ScriptExecution)
        .expect("UI thread may run scripts");
    guard
        .ensure_allowed(RuntimeGuardOperation::Rendering)
        .expect("UI thread may render");
}

#[test]
fn runtime_view_tree_preserves_parent_child_order_and_rejects_duplicates() {
    let root = RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column),
        RuntimeVisual::Fill(Color::rgba(8, 10, 14, 255)),
    );
    let header = RuntimeViewNode::new(
        RuntimeViewId::new("header"),
        LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(300.0, 32.0)),
        RuntimeVisual::Text(RuntimeTextVisual::new(
            "Hello Hawk2UI",
            18.0,
            Color::rgba(240, 244, 255, 255),
        )),
    );
    let meter = RuntimeViewNode::new(
        RuntimeViewId::new("meter"),
        LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(300.0, 48.0)),
        RuntimeVisual::Fill(Color::rgba(30, 144, 255, 255)),
    );

    let tree = RuntimeViewTree::new(root)
        .with_child(&RuntimeViewId::new("root"), header)
        .expect("header attaches to root")
        .with_child(&RuntimeViewId::new("root"), meter)
        .expect("meter attaches to root");

    assert_eq!(tree.root_id().as_str(), "root");
    assert_eq!(
        tree.children_of(&RuntimeViewId::new("root"))
            .iter()
            .map(RuntimeViewId::as_str)
            .collect::<Vec<_>>(),
        vec!["header", "meter"]
    );
    assert!(tree.node(&RuntimeViewId::new("header")).is_some());

    let duplicate = RuntimeViewNode::new(
        RuntimeViewId::new("meter"),
        LayoutStyle::custom_measured(),
        RuntimeVisual::None,
    );
    let error = tree
        .with_child(&RuntimeViewId::new("root"), duplicate)
        .expect_err("duplicate view IDs must be rejected");

    assert_eq!(error, RuntimeSceneError::DuplicateNode("meter".into()));
}

#[test]
fn runtime_scene_bridge_computes_layout_scene_and_paint_commands() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 200.0))
            .with_padding(BoxEdges::all(LayoutValue::px(8.0)))
            .with_gap(LayoutValue::px(4.0)),
        RuntimeVisual::Fill(Color::rgba(12, 14, 18, 255)),
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("title"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(200.0, 32.0)),
            RuntimeVisual::Text(RuntimeTextVisual::new(
                "Runtime Scene",
                16.0,
                Color::rgba(255, 255, 255, 255),
            )),
        ),
    )
    .expect("title attaches")
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("accent"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(120.0, 24.0)),
            RuntimeVisual::Fill(Color::rgba(255, 80, 48, 255)),
        ),
    )
    .expect("accent attaches");

    let frame = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0))
        .build(&tree)
        .expect("runtime view tree bridges into render data");

    assert_eq!(
        frame.geometry_for(&RuntimeViewId::new("root")).unwrap(),
        Geometry::new(0.0, 0.0, 320.0, 200.0)
    );
    assert_eq!(
        frame.geometry_for(&RuntimeViewId::new("title")).unwrap(),
        Geometry::new(8.0, 8.0, 200.0, 32.0)
    );
    assert_eq!(
        frame.geometry_for(&RuntimeViewId::new("accent")).unwrap(),
        Geometry::new(8.0, 44.0, 120.0, 24.0)
    );
    assert!(
        frame
            .scene()
            .node(&SceneNodeId::new("title"))
            .unwrap()
            .hit_test()
            .is_some()
    );
    assert_eq!(frame.draw_commands().len(), 3);
    assert_eq!(frame.paint_commands().commands().len(), 3);
    assert!(
        frame
            .paint_commands()
            .serialize_stable()
            .contains("draw-text:title:Runtime Scene")
    );
}

#[test]
fn runtime_scene_bridge_resolves_absolute_geometry_for_nested_nodes() {
    // Taffy reports each node's location relative to its parent's border box. A node whose parent is
    // the root is already at an absolute position (the root sits at the origin), so a depth-2 tree
    // never exposes the bug. This tree is depth-3: `panel` stacks below `spacer` at y=150, and `dot`
    // sits at `panel`'s content origin. `dot`'s parent-relative origin is therefore (0, 0), but its
    // absolute origin is (0, 150). The bridge must accumulate `panel`'s offset, because the wired
    // renderer blits each draw command verbatim and never walks the tree to add ancestor offsets.
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(400.0, 400.0)),
        RuntimeVisual::None,
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("spacer"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(400.0, 150.0)),
            RuntimeVisual::Fill(Color::rgba(20, 20, 20, 255)),
        ),
    )
    .expect("spacer attaches to root")
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("panel"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(400.0, 200.0)),
            RuntimeVisual::Fill(Color::rgba(40, 40, 40, 255)),
        ),
    )
    .expect("panel attaches to root")
    .with_child(
        &RuntimeViewId::new("panel"),
        RuntimeViewNode::new(
            RuntimeViewId::new("dot"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(40.0, 40.0)),
            RuntimeVisual::Fill(Color::rgba(255, 80, 48, 255)),
        ),
    )
    .expect("dot attaches to panel");

    let frame = RuntimeSceneBridge::new(Viewport::new(400.0, 400.0))
        .build(&tree)
        .expect("nested runtime view tree bridges into render data");

    // Depth-2 nodes are correct with or without accumulation (their parent is the root at origin).
    assert_eq!(
        frame.geometry_for(&RuntimeViewId::new("panel")).unwrap(),
        Geometry::new(0.0, 150.0, 400.0, 200.0)
    );

    // Depth-3: `dot` is at `panel`'s content origin, so its absolute geometry must equal `panel`'s
    // origin — NOT its parent-relative (0, 0). This is the regression guard for the nested-node bug.
    let dot = frame.geometry_for(&RuntimeViewId::new("dot")).unwrap();
    assert_eq!(
        dot,
        Geometry::new(0.0, 150.0, 40.0, 40.0),
        "nested node geometry must be absolute (parent offset accumulated), not parent-relative"
    );

    // The visible draw command for `dot` carries the same absolute geometry the renderer blits.
    let dot_command = frame
        .draw_commands()
        .iter()
        .find(|command| command.id().as_str() == "dot")
        .expect("dot produces a draw command");
    assert_eq!(dot_command.geometry(), dot);

    // The scene-graph hit-test rect (consumed by a11y/hit-test/invalidation) must also be absolute.
    let dot_hit_test = frame
        .scene()
        .node(&SceneNodeId::new("dot"))
        .expect("dot is present in the scene graph")
        .hit_test()
        .expect("dot has hit-test geometry");
    assert_eq!(dot_hit_test, dot);
}

#[test]
fn runtime_scene_bridge_uses_text_measurement_for_intrinsic_text_geometry() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(180.0, 96.0)),
        RuntimeVisual::None,
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("label"),
            LayoutStyle::custom_measured(),
            RuntimeVisual::Text(
                RuntimeTextVisual::new("Measured", 16.0, Color::rgba(255, 255, 255, 255))
                    .with_font_family("Atkinson"),
            ),
        ),
    )
    .expect("label attaches");

    let frame = RuntimeSceneBridge::new(Viewport::new(180.0, 96.0))
        .build_with_text_measurer(
            &tree,
            &TestTextMeasurer::new().with_average_glyph_width(8.0),
        )
        .expect("runtime text layout uses measurement");

    assert_eq!(
        frame.geometry_for(&RuntimeViewId::new("label")).unwrap(),
        Geometry::new(0.0, 0.0, 180.0, 19.0)
    );
    assert!(frame.draw_commands().iter().any(|command| matches!(
        command,
        RuntimeDrawCommand::Text {
            font_family,
            text,
            ..
        } if text == "Measured" && font_family == "Atkinson"
    )));
}

#[test]
fn runtime_scene_bridge_emits_compiled_asset_draw_commands_and_rejects_raw_paths() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(160.0, 96.0)),
        RuntimeVisual::None,
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("hero"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(32.0, 24.0)),
            RuntimeVisual::ImageAsset("hero".to_string()),
        ),
    )
    .expect("image asset attaches")
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("logo"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(32.0, 24.0)),
            RuntimeVisual::VectorAsset("logo".to_string()),
        ),
    )
    .expect("vector asset attaches");

    let frame = RuntimeSceneBridge::new(Viewport::new(160.0, 96.0))
        .build(&tree)
        .expect("asset visuals bridge into draw commands");

    assert!(frame.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::ImageAsset { asset_id, .. } if asset_id == "hero"
        )
    }));
    assert!(frame.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::VectorAsset { asset_id, .. } if asset_id == "logo"
        )
    }));

    let raw_path_tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(32.0, 24.0)),
        RuntimeVisual::ImageAsset("assets/hero.png".to_string()),
    ));
    let error = RuntimeSceneBridge::new(Viewport::new(160.0, 96.0))
        .build(&raw_path_tree)
        .expect_err("raw asset paths must not cross runtime render boundary");

    assert_eq!(error, RuntimeSceneError::InvalidNode("root".into()));
}

#[test]
fn runtime_scene_bridge_emits_shader_effect_draw_commands() {
    let effect = RuntimeShaderEffectVisual::new(
        "solid-red",
        "uniform float4 color; half4 main(float2 p) { return half4(color); }",
    )
    .with_uniform(ShaderEffectUniform::float4("color", [1.0, 0.0, 0.0, 1.0]))
    .with_child(ShaderEffectChildInput::image("mask", "noise"));
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(48.0, 24.0)),
        RuntimeVisual::ShaderEffect(effect),
    ));

    let frame = RuntimeSceneBridge::new(Viewport::new(80.0, 48.0))
        .build(&tree)
        .expect("shader effect visual bridges into a draw command");

    let command = frame
        .draw_commands()
        .iter()
        .find_map(|command| match command {
            RuntimeDrawCommand::ShaderEffect {
                id,
                geometry,
                effect,
            } => Some((id, geometry, effect)),
            _ => None,
        })
        .expect("shader effect command exists");

    assert_eq!(command.0.as_str(), "root");
    assert_eq!(*command.1, Geometry::new(0.0, 0.0, 48.0, 24.0));
    assert_eq!(command.2.effect_id(), "solid-red");
    assert_eq!(command.2.uniforms().len(), 1);
    assert_eq!(command.2.children().len(), 1);

    let invalid_tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(48.0, 24.0)),
        RuntimeVisual::ShaderEffect(RuntimeShaderEffectVisual::new(
            "",
            "uniform float4 color; half4 main(float2 p) { return half4(color); }",
        )),
    ));
    let error = RuntimeSceneBridge::new(Viewport::new(80.0, 48.0))
        .build(&invalid_tree)
        .expect_err("empty shader effect IDs must be rejected at runtime boundary");

    assert_eq!(error, RuntimeSceneError::InvalidNode("root".into()));
}

#[test]
fn runtime_scene_bridge_emits_custom_surface_draw_commands_with_realtime_data() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(160.0, 64.0)),
        RuntimeVisual::None,
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("meter"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(96.0, 24.0)),
            RuntimeVisual::CustomSurface(
                hawk2ui_runtime::RuntimeCustomSurfaceVisual::new(CustomSurfaceCategory::Meter)
                    .with_data_snapshot(
                        CustomSurfaceDataSnapshot::new([0.0, 0.5, 1.0])
                            .expect("valid realtime samples"),
                    )
                    .with_frame_interval(2)
                    .schedule_frame(4),
            ),
        ),
    )
    .expect("custom surface attaches")
    .invalidate(&RuntimeViewId::new("meter"))
    .expect("custom surface invalidates independently");

    let frame = RuntimeSceneBridge::new(Viewport::new(160.0, 64.0))
        .build(&tree)
        .expect("custom surface bridges into draw commands");

    let command = frame
        .draw_commands()
        .iter()
        .find_map(|command| match command {
            RuntimeDrawCommand::CustomSurface { surface, data, .. } => Some((surface, data)),
            _ => None,
        })
        .expect("custom surface command exists");
    assert_eq!(
        command.0.reserved_layout(),
        Geometry::new(0.0, 0.0, 96.0, 24.0)
    );
    assert!(command.0.is_frame_due(4));
    assert_eq!(command.1.samples(), &[0.0, 0.5, 1.0]);
    assert_eq!(
        frame
            .invalidated_view_ids()
            .iter()
            .map(RuntimeViewId::as_str)
            .collect::<Vec<_>>(),
        vec!["meter"]
    );
}

#[test]
fn runtime_scene_bridge_rejects_invalid_view_records_before_rendering() {
    let invalid_root = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new(""),
        LayoutStyle::flex_container(FlexDirection::Column),
        RuntimeVisual::Fill(Color::rgba(12, 14, 18, 255)),
    ));

    let error = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0))
        .build(&invalid_root)
        .expect_err("empty runtime view IDs must fail before layout");

    assert_eq!(error, RuntimeSceneError::InvalidNode(String::new()));

    let invalid_text = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column),
        RuntimeVisual::Text(RuntimeTextVisual::new(
            "Invalid",
            f32::NAN,
            Color::rgba(255, 255, 255, 255),
        )),
    ));

    let error = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0))
        .build(&invalid_text)
        .expect_err("invalid text metrics must fail before render commands");

    assert_eq!(error, RuntimeSceneError::InvalidNode("root".into()));
}

#[test]
fn runtime_scene_bridge_marks_invalidated_nodes_and_ancestors() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column),
        RuntimeVisual::Fill(Color::rgba(0, 0, 0, 255)),
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("meter"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(80.0, 20.0)),
            RuntimeVisual::Fill(Color::rgba(0, 200, 120, 255)),
        ),
    )
    .expect("meter attaches")
    .invalidate(&RuntimeViewId::new("meter"))
    .expect("meter invalidates");

    let frame = RuntimeSceneBridge::new(Viewport::new(100.0, 100.0))
        .build(&tree)
        .expect("invalidated tree bridges");

    assert_eq!(
        frame
            .invalidated_view_ids()
            .iter()
            .map(RuntimeViewId::as_str)
            .collect::<Vec<_>>(),
        vec!["meter"]
    );
    assert!(
        frame
            .scene()
            .node(&SceneNodeId::new("meter"))
            .unwrap()
            .invalidated()
    );
    assert!(
        frame
            .scene()
            .node(&SceneNodeId::new("root"))
            .unwrap()
            .invalidated()
    );
}

#[test]
fn runtime_scene_bridge_output_renders_visible_pixels_with_skia() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(180.0, 96.0))
            .with_padding(BoxEdges::all(LayoutValue::px(8.0)))
            .with_gap(LayoutValue::px(8.0)),
        RuntimeVisual::Fill(Color::rgba(8, 8, 12, 255)),
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("label"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(140.0, 28.0)),
            RuntimeVisual::Text(RuntimeTextVisual::new(
                "Pixels",
                18.0,
                Color::rgba(255, 255, 255, 255),
            )),
        ),
    )
    .expect("label attaches")
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("bar"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(96.0, 24.0)),
            RuntimeVisual::Fill(Color::rgba(240, 88, 40, 255)),
        ),
    )
    .expect("bar attaches");

    let frame = RuntimeSceneBridge::new(Viewport::new(180.0, 96.0))
        .build(&tree)
        .expect("runtime scene frame builds");
    let mut backend = SkiaRendererBackend::default();
    backend
        .create_surface("main", 180, 96)
        .expect("surface creates");
    backend.begin_frame("main").expect("frame begins");
    backend
        .clear(Color::rgba(0, 0, 0, 255))
        .expect("surface clears");
    render_frame_with_skia(&frame, &mut backend);
    backend.end_frame("main").expect("frame ends");

    let snapshot = backend.frame_snapshot("main").expect("snapshot exists");
    assert!(snapshot.pixels().contains(&0x00f0_5828));
    let changed_text_pixels = count_changed_pixels(
        snapshot,
        0x0008_080c,
        frame.geometry_for(&RuntimeViewId::new("label")).unwrap(),
    );
    assert!(
        changed_text_pixels > 0,
        "text draw should affect label pixels; changed={changed_text_pixels}, commands={:?}",
        backend.command_keys()
    );
}

fn render_frame_with_skia(frame: &RuntimeSceneFrame, backend: &mut SkiaRendererBackend) {
    backend
        .draw_runtime_scene_frame(frame, 0, 1.0)
        .expect("runtime scene frame renders");
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn count_changed_pixels(
    snapshot: &SkiaFrameSnapshot,
    background: u32,
    geometry: Geometry,
) -> usize {
    let start_x = geometry.x.max(0.0).floor() as u32;
    let start_y = geometry.y.max(0.0).floor() as u32;
    let end_x = (geometry.x + geometry.width)
        .ceil()
        .min(snapshot.width() as f32) as u32;
    let end_y = (geometry.y + geometry.height)
        .ceil()
        .min(snapshot.height() as f32) as u32;
    let mut changed = 0;
    for y in start_y..end_y {
        for x in start_x..end_x {
            if snapshot
                .pixel_at(x, y)
                .is_some_and(|pixel| pixel != background)
            {
                changed += 1;
            }
        }
    }
    changed
}

#[test]
fn runtime_scene_payload_rejects_deeply_nested_input_before_overflow() {
    let mut node = serde_json::json!({
        "id": "leaf",
        "width": 10.0,
        "height": 10.0,
        "visual": { "fill": [10, 20, 30, 255] },
        "children": []
    });
    for _ in 0..300 {
        node = serde_json::json!({
            "id": "branch",
            "width": 10.0,
            "height": 10.0,
            "visual": { "fill": [10, 20, 30, 255] },
            "children": [node]
        });
    }
    let payload = serde_json::json!({
        "viewport": { "width": 100.0, "height": 100.0 },
        "root": node
    });

    let error = hawk2ui_runtime::RuntimeScenePayload::from_json(&payload)
        .expect_err("a deeply nested scene payload is rejected before it can overflow the stack");

    assert_eq!(error.rule(), "runtime-scene.payload.too-deeply-nested");
}

#[test]
fn runtime_scene_payload_builds_a_well_formed_frame() {
    let payload = serde_json::json!({
        "viewport": { "width": 320.0, "height": 240.0 },
        "root": {
            "id": "root",
            "width": 320.0,
            "height": 240.0,
            "visual": { "fill": [255, 255, 255, 255] },
            "children": [
                {
                    "id": "panel",
                    "width": 100.0,
                    "height": 50.0,
                    "visual": { "fill": [10, 20, 30, 255] },
                    "children": []
                }
            ]
        }
    });

    let scene = hawk2ui_runtime::RuntimeScenePayload::from_json(&payload)
        .expect("a well-formed scene payload parses");
    scene
        .build_frame()
        .expect("a valid scene payload builds a runtime scene frame");
}

#[test]
fn runtime_scene_payload_rejects_unknown_fields() {
    let payload = serde_json::json!({
        "viewport": { "width": 100.0, "height": 100.0 },
        "root": {
            "id": "root",
            "width": 100.0,
            "height": 100.0,
            "visual": { "fill": [0, 0, 0, 255] },
            "children": [],
            "unexpected": true
        }
    });

    let error = hawk2ui_runtime::RuntimeScenePayload::from_json(&payload)
        .expect_err("unknown fields are rejected by deny_unknown_fields");

    assert_eq!(error.rule(), "runtime-scene.payload.parse-failed");
}
