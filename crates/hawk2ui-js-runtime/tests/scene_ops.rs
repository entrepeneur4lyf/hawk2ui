use hawk2ui_js_runtime::{
    RuntimeSceneOpAdapter, SceneMeasurementRequest, SceneNodeKind, SceneOp, SceneOpBatch,
    SceneValue,
};
use hawk2ui_layout::Viewport;
use hawk2ui_runtime::{RuntimeSceneBridge, RuntimeViewId, RuntimeVisual};

fn accessible_button_batch() -> SceneOpBatch {
    SceneOpBatch::new([
        SceneOp::CreateNode {
            id: "root".into(),
            kind: SceneNodeKind::View,
        },
        SceneOp::CreateNode {
            id: "cta".into(),
            kind: SceneNodeKind::Button,
        },
        SceneOp::SetAccessibility {
            id: "cta".into(),
            role: Some("button".into()),
            label: Some("Start render".into()),
            description: Some("Starts the offline render".into()),
            value: Some(SceneValue::String("ready".into())),
            disabled: Some(false),
            checked: None,
            pressed: Some(false),
            focused: Some(false),
        },
        SceneOp::AppendChild {
            parent: "root".into(),
            child: "cta".into(),
        },
        SceneOp::Commit,
    ])
}

#[test]
fn scene_op_batch_validates_and_round_trips() {
    let batch = SceneOpBatch::new([
        SceneOp::CreateNode {
            id: "root".into(),
            kind: SceneNodeKind::View,
        },
        SceneOp::CreateText {
            id: "title".into(),
            text: "Hello".into(),
        },
        SceneOp::SetAccessibility {
            id: "title".into(),
            role: Some("heading".into()),
            label: Some("Hello".into()),
            description: None,
            value: None,
            disabled: None,
            checked: None,
            pressed: None,
            focused: None,
        },
        SceneOp::FocusNode { id: "title".into() },
        SceneOp::MeasureNode {
            id: "title".into(),
            request: "initial-title-layout".into(),
        },
        SceneOp::AppendChild {
            parent: "root".into(),
            child: "title".into(),
        },
        SceneOp::Commit,
    ]);

    batch.validate().expect("batch validates");

    let encoded = serde_json::to_string(&batch).expect("batch serializes");
    assert!(
        encoded.contains("\"create-node\""),
        "serialized batch should use stable kebab-case op tags: {encoded}"
    );
    assert!(
        encoded.contains("\"set-accessibility\""),
        "serialized batch should include stable accessibility op tag: {encoded}"
    );
    assert!(
        encoded.contains("\"measure-node\""),
        "serialized batch should include stable measure op tag: {encoded}"
    );
    let decoded: SceneOpBatch = serde_json::from_str(&encoded).expect("batch deserializes");
    assert_eq!(decoded, batch);
}

#[test]
fn scene_op_batch_rejects_empty_ids() {
    let batch = SceneOpBatch::new([SceneOp::CreateNode {
        id: " ".into(),
        kind: SceneNodeKind::View,
    }]);

    let error = batch.validate().expect_err("empty id must be rejected");
    assert_eq!(error.rule(), "js-runtime.scene-op.invalid");
    assert!(error.message().contains("node id"));
}

#[test]
fn scene_op_batch_rejects_duplicate_creates() {
    let batch = SceneOpBatch::new([
        SceneOp::CreateNode {
            id: "node".into(),
            kind: SceneNodeKind::View,
        },
        SceneOp::CreateText {
            id: "node".into(),
            text: "duplicate".into(),
        },
    ]);

    let error = batch
        .validate()
        .expect_err("duplicate node id must be rejected");
    assert_eq!(error.rule(), "js-runtime.scene-op.invalid");
    assert!(error.message().contains("duplicate"));
}

#[test]
fn scene_op_batch_rejects_empty_event_names_and_handlers() {
    let batch = SceneOpBatch::new([SceneOp::RegisterEvent {
        id: "button".into(),
        event: "pointer.press".into(),
        handler: String::new(),
    }]);

    let error = batch
        .validate()
        .expect_err("empty handler must be rejected");
    assert_eq!(error.rule(), "js-runtime.scene-op.invalid");
    assert!(error.message().contains("handler id"));
}

#[test]
fn scene_op_batch_rejects_empty_measure_requests() {
    let batch = SceneOpBatch::new([SceneOp::MeasureNode {
        id: "field".into(),
        request: String::new(),
    }]);

    let error = batch
        .validate()
        .expect_err("empty measure request must be rejected");
    assert_eq!(error.rule(), "js-runtime.scene-op.invalid");
    assert!(error.message().contains("measure request"));
}

#[test]
fn scene_op_batch_rejects_unknown_node_kind_during_deserialization() {
    let error = serde_json::from_str::<SceneOpBatch>(
        r#"{"ops":[{"type":"create-node","id":"root","kind":"browser-div"}]}"#,
    )
    .expect_err("unknown node kind must be rejected");

    assert!(
        error.to_string().contains("browser-div"),
        "serde error should identify unknown node kind: {error}"
    );
}

#[test]
fn scene_op_adapter_applies_create_and_text_batches_to_runtime_tree() {
    let mut adapter = RuntimeSceneOpAdapter::default();

    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateNode {
                id: "count".into(),
                kind: SceneNodeKind::Text,
            },
            SceneOp::SetProp {
                id: "count".into(),
                name: "text".into(),
                value: SceneValue::String("0".into()),
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "count".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("initial scene ops apply");

    let tree = adapter.runtime_tree().expect("runtime tree exists");
    assert_eq!(tree.root_id().as_str(), "root");
    assert_eq!(
        tree.children_of(&RuntimeViewId::new("root"))
            .iter()
            .map(RuntimeViewId::as_str)
            .collect::<Vec<_>>(),
        vec!["count"]
    );
    let count = tree
        .node(&RuntimeViewId::new("count"))
        .expect("count node exists");
    let RuntimeVisual::Text(text) = count.visual() else {
        panic!("count should lower to a text visual");
    };
    assert_eq!(text.text(), "0");
}

#[test]
fn scene_op_adapter_records_accessibility_semantics_transactionally() {
    let mut adapter = RuntimeSceneOpAdapter::default();
    adapter
        .apply_batch(&accessible_button_batch())
        .expect("accessibility scene ops apply");

    let semantics = adapter
        .accessibility_semantics("cta")
        .expect("button semantics are retained");
    assert_eq!(semantics.role.as_deref(), Some("button"));
    assert_eq!(semantics.label.as_deref(), Some("Start render"));
    assert_eq!(
        semantics.description.as_deref(),
        Some("Starts the offline render")
    );
    assert_eq!(semantics.value, Some(SceneValue::String("ready".into())));
    assert_eq!(semantics.disabled, Some(false));
    assert_eq!(semantics.pressed, Some(false));

    let error = adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::SetAccessibility {
                id: "missing".into(),
                role: Some("button".into()),
                label: None,
                description: None,
                value: None,
                disabled: None,
                checked: None,
                pressed: None,
                focused: None,
            },
            SceneOp::Commit,
        ]))
        .expect_err("missing node semantics update should fail");
    assert_eq!(error.rule(), "js-runtime.scene-tree.apply-failed");
    assert!(
        adapter.accessibility_semantics("missing").is_none(),
        "failed batch must not leak staged accessibility semantics"
    );
    assert!(
        adapter.accessibility_semantics("cta").is_some(),
        "failed batch must not corrupt existing accessibility semantics"
    );
}

#[test]
fn scene_op_adapter_clears_accessibility_semantics() {
    let mut adapter = RuntimeSceneOpAdapter::default();
    adapter
        .apply_batch(&accessible_button_batch())
        .expect("accessibility scene ops apply");
    assert!(adapter.accessibility_semantics("cta").is_some());

    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::SetAccessibility {
                id: "cta".into(),
                role: None,
                label: None,
                description: None,
                value: None,
                disabled: None,
                checked: None,
                pressed: None,
                focused: None,
            },
            SceneOp::Commit,
        ]))
        .expect("empty accessibility update applies");

    assert!(
        adapter.accessibility_semantics("cta").is_none(),
        "empty accessibility update should clear retained semantics"
    );
}

#[test]
fn scene_op_adapter_tracks_focus_transactionally() {
    let mut adapter = RuntimeSceneOpAdapter::default();
    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateNode {
                id: "field".into(),
                kind: SceneNodeKind::Input,
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "field".into(),
            },
            SceneOp::FocusNode { id: "field".into() },
            SceneOp::Commit,
        ]))
        .expect("focus scene ops apply");

    assert_eq!(adapter.focused_node(), Some("field"));

    let error = adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::FocusNode {
                id: "missing".into(),
            },
            SceneOp::Commit,
        ]))
        .expect_err("missing focused node should fail");
    assert_eq!(error.rule(), "js-runtime.scene-tree.apply-failed");
    assert_eq!(
        adapter.focused_node(),
        Some("field"),
        "failed focus batch must not corrupt retained focus"
    );
}

#[test]
fn scene_op_adapter_tracks_measure_requests_transactionally() {
    let mut adapter = RuntimeSceneOpAdapter::default();
    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateNode {
                id: "field".into(),
                kind: SceneNodeKind::Input,
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "field".into(),
            },
            SceneOp::MeasureNode {
                id: "field".into(),
                request: "field-layout".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("measure scene ops apply");

    assert_eq!(
        adapter.measurement_requests(),
        &[SceneMeasurementRequest {
            node_id: "field".into(),
            request: "field-layout".into(),
        }]
    );

    let error = adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::MeasureNode {
                id: "missing".into(),
                request: "missing-layout".into(),
            },
            SceneOp::Commit,
        ]))
        .expect_err("missing measured node should fail");
    assert_eq!(error.rule(), "js-runtime.scene-tree.apply-failed");
    assert_eq!(
        adapter.measurement_requests(),
        &[SceneMeasurementRequest {
            node_id: "field".into(),
            request: "field-layout".into(),
        }],
        "failed measure batch must not corrupt retained measurement requests"
    );
}

#[test]
fn scene_op_adapter_updates_text_transactionally() {
    let mut adapter = RuntimeSceneOpAdapter::default();
    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateNode {
                id: "count".into(),
                kind: SceneNodeKind::Text,
            },
            SceneOp::SetProp {
                id: "count".into(),
                name: "text".into(),
                value: SceneValue::String("0".into()),
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "count".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("initial scene ops apply");

    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::SetProp {
                id: "count".into(),
                name: "text".into(),
                value: SceneValue::String("1".into()),
            },
            SceneOp::Commit,
        ]))
        .expect("text update applies");

    let error = adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::SetProp {
                id: "missing".into(),
                name: "text".into(),
                value: SceneValue::String("corrupt".into()),
            },
            SceneOp::Commit,
        ]))
        .expect_err("missing node update should fail");
    assert_eq!(error.rule(), "js-runtime.scene-tree.apply-failed");

    let count = adapter
        .runtime_tree()
        .and_then(|tree| tree.node(&RuntimeViewId::new("count")))
        .expect("count node remains");
    let RuntimeVisual::Text(text) = count.visual() else {
        panic!("count should remain a text visual");
    };
    assert_eq!(
        text.text(),
        "1",
        "failed batch must not corrupt runtime tree"
    );

    let frame = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0))
        .build(adapter.runtime_tree().expect("runtime tree remains"))
        .expect("updated tree builds a runtime scene frame");
    assert!(
        frame
            .invalidated_view_ids()
            .iter()
            .any(|id| id.as_str() == "count"),
        "text update should invalidate the runtime node for render/layout scheduling"
    );
}

#[test]
fn scene_op_adapter_lowers_input_value_to_text_visual() {
    let mut adapter = RuntimeSceneOpAdapter::default();
    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateNode {
                id: "field".into(),
                kind: SceneNodeKind::Input,
            },
            SceneOp::SetProp {
                id: "field".into(),
                name: "value".into(),
                value: SceneValue::String("Ada".into()),
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "field".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("initial input value scene ops apply");

    let field = adapter
        .runtime_tree()
        .and_then(|tree| tree.node(&RuntimeViewId::new("field")))
        .expect("field node exists");
    let RuntimeVisual::Text(text) = field.visual() else {
        panic!("field should lower controlled value to a text visual");
    };
    assert_eq!(text.text(), "Ada");

    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::SetProp {
                id: "field".into(),
                name: "value".into(),
                value: SceneValue::String("Grace".into()),
            },
            SceneOp::Commit,
        ]))
        .expect("updated input value scene ops apply");

    let field = adapter
        .runtime_tree()
        .and_then(|tree| tree.node(&RuntimeViewId::new("field")))
        .expect("field node remains");
    let RuntimeVisual::Text(text) = field.visual() else {
        panic!("field should remain a text visual");
    };
    assert_eq!(text.text(), "Grace");
}

#[test]
fn scene_op_adapter_moves_existing_children_with_append_ops() {
    let mut adapter = RuntimeSceneOpAdapter::default();
    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateNode {
                id: "alpha".into(),
                kind: SceneNodeKind::Text,
            },
            SceneOp::SetProp {
                id: "alpha".into(),
                name: "text".into(),
                value: SceneValue::String("Alpha".into()),
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "alpha".into(),
            },
            SceneOp::CreateNode {
                id: "beta".into(),
                kind: SceneNodeKind::Text,
            },
            SceneOp::SetProp {
                id: "beta".into(),
                name: "text".into(),
                value: SceneValue::String("Beta".into()),
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "beta".into(),
            },
            SceneOp::CreateNode {
                id: "gamma".into(),
                kind: SceneNodeKind::Text,
            },
            SceneOp::SetProp {
                id: "gamma".into(),
                name: "text".into(),
                value: SceneValue::String("Gamma".into()),
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "gamma".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("initial keyed children apply");

    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "alpha".into(),
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "beta".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("existing children move to the end");

    let tree = adapter.runtime_tree().expect("runtime tree remains");
    assert_eq!(
        tree.children_of(&RuntimeViewId::new("root"))
            .iter()
            .map(RuntimeViewId::as_str)
            .collect::<Vec<_>>(),
        vec!["gamma", "alpha", "beta"]
    );
}

#[test]
fn scene_op_adapter_reparents_existing_child_without_disposing_subtree() {
    let mut adapter = RuntimeSceneOpAdapter::default();
    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateNode {
                id: "left".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "left".into(),
            },
            SceneOp::CreateNode {
                id: "right".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "right".into(),
            },
            SceneOp::CreateNode {
                id: "item".into(),
                kind: SceneNodeKind::Button,
            },
            SceneOp::SetProp {
                id: "item".into(),
                name: "text".into(),
                value: SceneValue::String("Move me".into()),
            },
            SceneOp::RegisterEvent {
                id: "item".into(),
                event: "pointer.press".into(),
                handler: "moveItem".into(),
            },
            SceneOp::AppendChild {
                parent: "left".into(),
                child: "item".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("initial parented child applies");

    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::RemoveChild {
                parent: "left".into(),
                child: "item".into(),
            },
            SceneOp::AppendChild {
                parent: "right".into(),
                child: "item".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("existing child reparents without disposal");

    let tree = adapter.runtime_tree().expect("runtime tree remains");
    assert!(tree.children_of(&RuntimeViewId::new("left")).is_empty());
    assert_eq!(
        tree.children_of(&RuntimeViewId::new("right"))
            .iter()
            .map(RuntimeViewId::as_str)
            .collect::<Vec<_>>(),
        vec!["item"]
    );
    let item = tree
        .node(&RuntimeViewId::new("item"))
        .expect("reparented item remains");
    let RuntimeVisual::Text(text) = item.visual() else {
        panic!("reparented item should preserve its text visual");
    };
    assert_eq!(text.text(), "Move me");
    assert_eq!(
        adapter.event_handler("item", "pointer.press"),
        Some("moveItem"),
        "detach-only reparenting should preserve event handlers"
    );
}

#[test]
fn scene_op_adapter_removes_subtree_and_handlers() {
    let mut adapter = RuntimeSceneOpAdapter::default();
    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".into(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateNode {
                id: "increment".into(),
                kind: SceneNodeKind::Button,
            },
            SceneOp::RegisterEvent {
                id: "increment".into(),
                event: "pointer.press".into(),
                handler: "increment".into(),
            },
            SceneOp::AppendChild {
                parent: "root".into(),
                child: "increment".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("button scene ops apply");

    assert_eq!(
        adapter.event_handler("increment", "pointer.press"),
        Some("increment")
    );

    adapter
        .apply_batch(&SceneOpBatch::new([
            SceneOp::RemoveChild {
                parent: "root".into(),
                child: "increment".into(),
            },
            SceneOp::DisposeSubtree {
                id: "increment".into(),
            },
            SceneOp::Commit,
        ]))
        .expect("subtree removal applies");

    let tree = adapter.runtime_tree().expect("runtime tree remains");
    assert!(tree.node(&RuntimeViewId::new("increment")).is_none());
    assert_eq!(adapter.event_handler("increment", "pointer.press"), None);
}
