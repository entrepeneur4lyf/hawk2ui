//! Stable operation key formatting shared by authoring adapters.

use crate::{ElementId, ElementKind, EventBinding, EventKind, HandlerRef, NativeLifecycleEvent};

pub(crate) fn mount_element_key(id: &ElementId) -> String {
    format!("mount-element:{}", id.as_str())
}

pub(crate) fn mount_component_key(id: &str) -> String {
    format!("mount-component:{id}")
}

pub(crate) fn declare_surface_key(id: &str) -> String {
    format!("declare-surface:{id}")
}

pub(crate) fn bind_event_key(binding: &EventBinding) -> String {
    bind_event_parts_key(binding.target(), binding.event())
}

pub(crate) fn bind_event_parts_key(target: &ElementId, event: &EventKind) -> String {
    format!("bind-event:{}:{}", target.as_str(), event.stable_key())
}

pub(crate) fn native_lifecycle_key(
    event: NativeLifecycleEvent,
    target: &ElementId,
    handler: &HandlerRef,
) -> String {
    format!(
        "lifecycle:{}:{}:{}",
        lifecycle_key(event),
        target.as_str(),
        handler.as_str()
    )
}

pub(crate) fn create_node_key(id: &ElementId, kind: ElementKind) -> String {
    format!("create-node:{}:{}", id.as_str(), element_kind_key(kind))
}

pub(crate) fn set_prop_key(id: &ElementId, name: &str) -> String {
    format!("set-prop:{}:{name}", id.as_str())
}

pub(crate) fn set_style_key(id: &ElementId, style_name: &str) -> String {
    format!("set-style:{}:{style_name}", id.as_str())
}

pub(crate) fn set_asset_key(id: &ElementId, asset_path: &str) -> String {
    format!("set-asset:{}:{asset_path}", id.as_str())
}

pub(crate) fn set_ref_key(id: &ElementId, reference_name: &str) -> String {
    format!("set-ref:{}:{reference_name}", id.as_str())
}

pub(crate) fn bind_lifecycle_key(
    id: &ElementId,
    event: NativeLifecycleEvent,
    handler: &HandlerRef,
) -> String {
    format!(
        "bind-lifecycle:{}:{}:{}",
        id.as_str(),
        lifecycle_key(event),
        handler.as_str()
    )
}

pub(crate) fn append_child_key(parent: &ElementId, child: &ElementId, key: Option<&str>) -> String {
    match key {
        Some(key) => format!(
            "append-child:{}:{}:key:{key}",
            parent.as_str(),
            child.as_str()
        ),
        None => format!("append-child:{}:{}", parent.as_str(), child.as_str()),
    }
}

pub(crate) fn error_boundary_key(id: &ElementId, handler: &HandlerRef) -> String {
    format!("error-boundary:{}:{}", id.as_str(), handler.as_str())
}

pub(crate) fn commit_key(root: &ElementId) -> String {
    format!("commit:{}", root.as_str())
}

pub(crate) fn remove_node_key(id: &ElementId) -> String {
    format!("remove-node:{}", id.as_str())
}

pub(crate) const fn element_kind_key(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::View => "view",
        ElementKind::Text => "text",
        ElementKind::Button => "button",
        ElementKind::CustomSurface => "custom-surface",
    }
}

pub(crate) const fn lifecycle_key(event: NativeLifecycleEvent) -> &'static str {
    match event {
        NativeLifecycleEvent::Mounted => "mounted",
        NativeLifecycleEvent::Suspended => "suspended",
        NativeLifecycleEvent::Resumed => "resumed",
        NativeLifecycleEvent::HotReloaded => "hot-reloaded",
        NativeLifecycleEvent::ErrorBoundary => "error-boundary",
        NativeLifecycleEvent::Shutdown => "shutdown",
        NativeLifecycleEvent::Unmounted => "unmounted",
    }
}
