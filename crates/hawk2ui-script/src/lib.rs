#![forbid(unsafe_code)]
//! Production script backend for `Hawk2UI` `JavaScript` and `TypeScript` execution.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Write as _},
    path::Path,
};

use boa_engine::{Context, JsValue, JsVariant, Source};
use hawk2ui_api::Diagnostic;
use hawk2ui_authoring::{
    FrameworkDynamicBinding, NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError, PropValue,
};
use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{HelperLoaderMode, TransformOptions, Transformer};
use serde::{Deserialize, Serialize};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-script";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Script module kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptModuleKind {
    /// JavaScript module.
    JavaScript,
    /// TypeScript source that has been compiled to JavaScript before execution.
    TypeScript,
}

/// Script module input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptModule {
    id: String,
    source: String,
    kind: ScriptModuleKind,
}

impl ScriptModule {
    /// Creates a JavaScript module.
    #[must_use]
    pub fn javascript(id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            kind: ScriptModuleKind::JavaScript,
        }
    }

    /// Creates a TypeScript module.
    #[must_use]
    pub fn typescript(id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            kind: ScriptModuleKind::TypeScript,
        }
    }

    /// Creates a module whose kind is inferred from `source_path`'s extension:
    /// `.ts`/`.tsx` produce a TypeScript module, everything else JavaScript. The
    /// path doubles as the module id.
    #[must_use]
    pub fn for_source_path(source_path: &str, source: impl Into<String>) -> Self {
        let is_typescript = std::path::Path::new(source_path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("ts") || extension.eq_ignore_ascii_case("tsx")
            });
        if is_typescript {
            Self::typescript(source_path, source)
        } else {
            Self::javascript(source_path, source)
        }
    }

    /// Returns the module kind.
    #[must_use]
    pub const fn kind(&self) -> ScriptModuleKind {
        self.kind
    }
}

/// Wraps a compiled entry module so its exported `mount` function is invoked
/// under a host object and its result is serialized to a JSON node-tree string
/// — the convention an entry script's `mount(host)` follows. Returns `None` when
/// the source declares no `mount` function (the caller then falls back, e.g. to
/// a visible-title probe).
///
/// Both the desktop host and the plugin editor run the same entry script, so
/// they share this convention rather than each reinventing it. This basic
/// bootstrap exposes an empty, compatibility host; richer host projections use
/// [`entry_mount_bootstrap_with_host`].
#[must_use]
pub fn entry_mount_bootstrap(source: &str) -> Option<String> {
    let source = source.replacen("export function mount", "function mount", 1);
    if !source.contains("function mount") {
        return None;
    }
    Some(format!(
        r#"{source}

const __hawk2ui_host = Object.freeze({{
    events: Object.freeze([]),
    on(_name, handler) {{
        if (typeof handler !== "function") {{
            throw new Error("hawk2ui: host.on requires a function handler");
        }}
    }},
    setState(_value) {{}}
}});

JSON.stringify(mount(__hawk2ui_host));
"#
    ))
}

/// The value kind of a projected parameter, mirrored to editor JS so an author
/// sees a `bool` as a boolean, an `enum` as its variant index, an `int` as an
/// integer — never every kind flattened to a float.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HostParamKind {
    /// Continuous floating-point parameter.
    Float,
    /// Discrete integer parameter.
    Int,
    /// Boolean parameter.
    Bool,
    /// Indexed-choice (enum) parameter.
    Enum,
}

/// A projected parameter's current value, serialized to the JS scalar matching
/// its kind (`Float`/`Int` → number, `Bool` → boolean, `Enum` → variant index).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum HostParamValue {
    /// Plain floating-point value.
    Float(f64),
    /// Plain integer value.
    Int(i64),
    /// Boolean value.
    Bool(bool),
    /// Indexed-choice variant index.
    Enum(u32),
}

/// One parameter projected into an editor entry script, addressed by its stable
/// string key. The numeric truce `id` is carried as the host-side routing detail
/// that maps a write back to the bridge, but authors address parameters by
/// `key`, never by id.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HostParam {
    /// Stable string key the author addresses the parameter by.
    pub key: String,
    /// truce `ParamId` u32, verbatim — the wire detail the host maps a write
    /// (`host.setParam(key, …)`) back onto the bridge with. Authors use `key`.
    pub id: u32,
    /// Value kind, which drives the JS type of `value`.
    pub kind: HostParamKind,
    /// Current value, typed by `kind`.
    pub value: HostParamValue,
    /// Normalized value in `0.0..=1.0`.
    pub normalized: f64,
    /// Host-formatted display text (value plus unit).
    pub text: String,
    /// Variant display names for an enum parameter; empty for every other kind.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
}

/// One read-only meter projected into an editor entry script.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HostMeter {
    /// Stable string key the author addresses the meter by.
    pub key: String,
    /// truce meter `ParamId` u32 (`METER_ID_BASE + declaration_index`), carried
    /// so JS never computes it. Meters are read-only.
    pub id: u32,
    /// Current level in `0.0..=1.0`.
    pub value: f32,
}

/// The frozen snapshot of parameters and meters projected into an editor entry
/// script's `mount(host)` call. Reads are pure data — no host call — so the
/// script runs under the same `HostCallPolicy::deny_all` as the no-host path.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct HostSnapshot {
    /// Projected parameters, in declaration order.
    pub params: Vec<HostParam>,
    /// Projected read-only meters, in declaration order.
    pub meters: Vec<HostMeter>,
}

/// Wraps a compiled entry module like [`entry_mount_bootstrap`], projecting a
/// `host` that carries a frozen `snapshot` of parameters and meters the script
/// reads by string key, accepts parameter edits, and threads an opaque UI-state
/// blob across invocations.
///
/// Reads (pure data — embedded as a JSON literal, no host call, `deny_all`
/// preserved):
///
/// ```ignore
/// host.param("cutoff").value      // typed by kind (number / boolean / index)
/// host.param("cutoff").normalized // 0..1
/// host.param("cutoff").text       // host-formatted display string
/// host.param("cutoff").id         // truce ParamId u32 (wire detail; use the key)
/// host.params                     // all params, declaration order
/// host.meter("out")               // a level, 0..1
/// host.meters                     // all meters
/// ```
///
/// Writes — imperative verbs that record an **ordered** edit list the host
/// replays onto truce's bridge (`begin_edit`/`set_param`/`end_edit`) on the UI
/// thread. Edits ride the return JSON, so no host-call capability is added:
///
/// ```ignore
/// host.beginEdit("cutoff")             // {op:"begin"}
/// host.setParam("cutoff", 0.55)        // {op:"set", normalized}
/// host.setParamPlain("cutoff", 1200)   // {op:"setPlain", plain} — host normalizes via the range
/// host.endEdit("cutoff")               // {op:"end"}
/// host.automate("bypass", 1.0)         // {op:"automate"} — one-shot begin+set+end
/// ```
///
/// UI/gesture state is threaded, not held in JS (the execution model is
/// stateless): `incoming_ui` (a JSON value, `"null"` if none) is exposed as
/// `host.ui`; `host.setUi(value)` sets the outgoing blob, which defaults to the
/// incoming one when untouched. The host persists it and re-embeds it next
/// invocation. This is **not** truce's `set_state` (the plugin's custom state) —
/// it is the editor's own gesture/drag bookkeeping.
///
/// The entry returns the C2b view tree; this bootstrap wraps it into the locked
/// wire shape `{ tree, edits, ui }`, `JSON.stringify`d as `execution.value()`.
/// Parse it with [`parse_entry_envelope`]. Reads and writes both throw on an
/// unknown key. Returns `None` when the source declares no `mount` function.
///
/// `incoming_ui` must be a valid JSON value; the host produces it by
/// re-serializing the prior invocation's [`EntryEnvelope::ui_json`] (`serde_json`
/// output, so injection-safe). An empty string is treated as `null`.
#[must_use]
pub fn entry_mount_bootstrap_with_host(
    source: &str,
    snapshot: &HostSnapshot,
    events: &[FrameInput],
    incoming_ui: &str,
) -> Option<String> {
    let source = source.replacen("export function mount", "function mount", 1);
    if !source.contains("function mount") {
        return None;
    }
    // serde_json produces a valid, injection-safe JS expression (JSON ⊂ JS),
    // correctly escaping author-controlled keys and display text. A failure is
    // unreachable for this plain data, but falls back to an empty snapshot
    // rather than panicking (the crate forbids `unwrap`/`expect` in non-test).
    let snapshot = serde_json::to_string(snapshot)
        .unwrap_or_else(|_| String::from(r#"{"params":[],"meters":[]}"#));
    // The per-frame input batch uses the same injection-safe JSON-⊂-JS embedding
    // as the snapshot. An empty batch is `[]` — an idle frame the entry sees as
    // `host.events.length === 0`.
    let events = serde_json::to_string(events).unwrap_or_else(|_| String::from("[]"));
    // The host always feeds serde_json output (or "null"), so this is a valid,
    // injection-safe JS expression. An empty string degrades to `null`.
    let incoming_ui = if incoming_ui.trim().is_empty() {
        "null"
    } else {
        incoming_ui
    };
    Some(format!(
        r#"{source}

const __hawk2ui_snapshot = {snapshot};
const __hawk2ui_params_by_key = {{}};
for (const __param of __hawk2ui_snapshot.params) {{
    __hawk2ui_params_by_key[__param.key] = Object.freeze(__param);
}}
const __hawk2ui_meters_by_key = {{}};
for (const __meter of __hawk2ui_snapshot.meters) {{
    __hawk2ui_meters_by_key[__meter.key] = __meter.value;
}}
  function __hawk2ui_require_param(key) {{
      if (__hawk2ui_params_by_key[key] === undefined) {{
          throw new Error("hawk2ui: unknown parameter '" + key + "'");
      }}
  }}
  function __hawk2ui_dispatch_event(name, handler) {{
      if (typeof handler !== "function") {{
          throw new Error("hawk2ui: host.on requires a function handler");
      }}
      const __wanted = String(name);
      for (const __event of __hawk2ui_events) {{
          if (__wanted === "input" || __wanted === "*" || __wanted === __event.kind) {{
              handler(Object.freeze(__event));
          }}
      }}
  }}
  const __hawk2ui_edits = [];
  const __hawk2ui_events = {events};
  const __hawk2ui_ui_in = {incoming_ui};
let __hawk2ui_ui_out = __hawk2ui_ui_in;
const __hawk2ui_host = Object.freeze({{
      // Per-frame input: the events that arrived since the previous frame,
      // drained and in arrival order; `[]` on an idle frame.
      events: Object.freeze(__hawk2ui_events.map(Object.freeze)),
      on(name, handler) {{ __hawk2ui_dispatch_event(name, handler); }},
      setState(_value) {{}},
    params: Object.freeze(__hawk2ui_snapshot.params.map(Object.freeze)),
    param(key) {{
        const __found = __hawk2ui_params_by_key[key];
        if (__found === undefined) {{
            throw new Error("hawk2ui: unknown parameter '" + key + "'");
        }}
        return __found;
    }},
    meters: Object.freeze(__hawk2ui_snapshot.meters.map(Object.freeze)),
    meter(key) {{
        const __level = __hawk2ui_meters_by_key[key];
        if (__level === undefined) {{
            throw new Error("hawk2ui: unknown meter '" + key + "'");
        }}
        return __level;
    }},
    ui: Object.freeze(__hawk2ui_ui_in),
    setUi(value) {{ __hawk2ui_ui_out = value; }},
    beginEdit(key) {{ __hawk2ui_require_param(key); __hawk2ui_edits.push({{ op: "begin", key: key }}); }},
    endEdit(key) {{ __hawk2ui_require_param(key); __hawk2ui_edits.push({{ op: "end", key: key }}); }},
    setParam(key, normalized) {{ __hawk2ui_require_param(key); __hawk2ui_edits.push({{ op: "set", key: key, normalized: normalized }}); }},
    setParamPlain(key, plain) {{ __hawk2ui_require_param(key); __hawk2ui_edits.push({{ op: "setPlain", key: key, plain: plain }}); }},
    automate(key, normalized) {{ __hawk2ui_require_param(key); __hawk2ui_edits.push({{ op: "automate", key: key, normalized: normalized }}); }}
}});

const __hawk2ui_tree = mount(__hawk2ui_host);
JSON.stringify({{ tree: __hawk2ui_tree, edits: __hawk2ui_edits, ui: __hawk2ui_ui_out }});
"#
    ))
}

/// One parameter edit recorded by an editor entry script's write verbs
/// (`host.beginEdit`/`setParam`/`setParamPlain`/`endEdit`/`automate`), parsed
/// from the return envelope's `edits` array. The host validates each, maps `key`
/// → truce `ParamId`, and replays the gesture onto the bridge; meters have no
/// edit variant because meters are read-only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum HostEdit {
    /// Start an automation gesture (`begin_edit`).
    Begin {
        /// Stable parameter key.
        key: String,
    },
    /// Set the parameter to a normalized `0.0..=1.0` value (`set_param`).
    Set {
        /// Stable parameter key.
        key: String,
        /// Normalized value, `0.0..=1.0`.
        normalized: f64,
    },
    /// Set the parameter to a plain (natural-unit) value; the host normalizes it
    /// via the parameter's range before `set_param`.
    SetPlain {
        /// Stable parameter key.
        key: String,
        /// Plain (denormalized) value.
        plain: f64,
    },
    /// One-shot edit: `begin_edit` + `set_param` + `end_edit` (`automate`).
    Automate {
        /// Stable parameter key.
        key: String,
        /// Normalized value, `0.0..=1.0`.
        normalized: f64,
    },
    /// End the automation gesture (`end_edit`).
    End {
        /// Stable parameter key.
        key: String,
    },
}

/// One window input event projected into an editor entry's per-frame
/// `host.events` array. The host drains the events that arrived since the
/// previous frame, translates each from a `PluginHostEvent`
/// (pointer/keyboard/focus only; resize/DPI/lifecycle are engine-handled), and
/// embeds them as a frozen array each invocation — input rides the source, like
/// the read snapshot, so it adds no host-call capability.
///
/// The `kind` tag (not `type`, which is the view-node tag) discriminates the
/// variants; coordinates are logical points sharing the layout geometry space
/// so an author hit-tests `x`/`y` against node positions directly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FrameInput {
    /// A pointer event. `button` is the single rich label the host emits (Fork
    /// B → B1): `"move"`, `"<btn>-down"`/`"<btn>-up"` (btn ∈ left/right/middle),
    /// `"scroll-up"`/`"scroll-down"`, `"enter"`/`"leave"`, and the drag-and-drop
    /// labels `"drag-entered"`/`"drag-moved"`/`"drag-left"`/`"drag-dropped"`.
    Pointer {
        /// Logical x coordinate, top-left origin (D4).
        x: f64,
        /// Logical y coordinate, top-left origin (D4).
        y: f64,
        /// Rich button/action label (see variant docs).
        button: String,
    },
    /// A keyboard event.
    Key {
        /// Physical or logical key label.
        key: String,
        /// Whether the key is pressed (`true`) or released (`false`).
        pressed: bool,
    },
    /// An editor focus change.
    Focus {
        /// Whether the editor gained (`true`) or lost (`false`) focus.
        focused: bool,
    },
}

/// The parsed `{ tree, edits, ui }` envelope an editor entry returns (see
/// [`entry_mount_bootstrap_with_host`] and [`parse_entry_envelope`]).
#[derive(Clone, Debug, PartialEq)]
pub struct EntryEnvelope {
    /// The view tree as a JSON string, ready for `EntryNode::from_tree_json`.
    pub tree_json: String,
    /// The ordered parameter edits the script emitted this invocation.
    pub edits: Vec<HostEdit>,
    /// The author's outgoing UI-state blob as a JSON string; the host persists
    /// it and re-embeds it as `incoming_ui` next invocation.
    pub ui_json: String,
}

/// Error parsing an editor entry's return envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvelopeError {
    message: String,
}

impl EnvelopeError {
    /// The human-readable parse failure.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid editor entry envelope: {}", self.message)
    }
}

impl std::error::Error for EnvelopeError {}

/// Parses an editor entry's `execution.value()` string into its [`EntryEnvelope`]
/// (the locked `{ tree, edits, ui }` wire shape).
///
/// `tree` is required (the entry must return a view tree); `edits` and `ui`
/// default to empty / `null` when absent, so a read-only entry that calls no
/// write verb parses cleanly. The `tree` and `ui` sub-values are re-serialized to
/// strings so callers downstream (e.g. `hawk2ui-plugin-truce`'s scene build, the
/// host's UI-state persistence) need no JSON dependency of their own.
///
/// # Errors
///
/// Returns an [`EnvelopeError`] when `value` is not the expected envelope object
/// (malformed JSON, a missing `tree`, or an unrecognized edit op).
pub fn parse_entry_envelope(value: &str) -> Result<EntryEnvelope, EnvelopeError> {
    #[derive(Deserialize)]
    struct RawEnvelope {
        tree: serde_json::Value,
        #[serde(default)]
        edits: Vec<HostEdit>,
        #[serde(default)]
        ui: serde_json::Value,
    }

    let raw: RawEnvelope = serde_json::from_str(value).map_err(|error| EnvelopeError {
        message: error.to_string(),
    })?;
    let tree_json = serde_json::to_string(&raw.tree).map_err(|error| EnvelopeError {
        message: error.to_string(),
    })?;
    let ui_json = serde_json::to_string(&raw.ui).map_err(|error| EnvelopeError {
        message: error.to_string(),
    })?;
    Ok(EntryEnvelope {
        tree_json,
        edits: raw.edits,
        ui_json,
    })
}

/// Per-parameter write routing the host needs to replay an edit list onto the
/// truce bridge: the `key` → `id` map plus the data to normalize a `setParamPlain`
/// (the bridge offers only a normalized `set_param`, so the host owns the
/// plain→normalized conversion). Built from the parameter model by
/// `hawk2ui-build`'s `edit_routing_from_model`; consumed by the editor's replay.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamRoute {
    /// Stable string key the author addresses the parameter by.
    pub key: String,
    /// truce `ParamId` u32 the edit is replayed onto.
    pub id: u32,
    /// Value kind, selecting how a plain value normalizes.
    pub kind: HostParamKind,
    /// Range minimum (float/int); unused for bool/enum.
    pub min: f64,
    /// Range maximum (float/int); unused for bool/enum.
    pub max: f64,
    /// Variant count for an enum (for the `index/(count - 1)` normalization);
    /// `0` for every other kind.
    pub variant_count: u32,
}

impl ParamRoute {
    /// Normalizes a plain (natural-unit) value to `0.0..=1.0` for the bridge's
    /// `set_param`, mirroring `ParameterRecord::normalize` (float/int by range,
    /// bool by non-zero) plus the enum `index / (count - 1)` of the snapshot
    /// projection. The host owns this conversion; the math is duplicated here rather
    /// than imported because `hawk2ui-plugin-truce` stays free of the parameter
    /// model.
    #[must_use]
    pub fn normalize_plain(&self, plain: f64) -> f64 {
        let normalized = match self.kind {
            HostParamKind::Bool => {
                if plain.abs() > f64::EPSILON {
                    1.0
                } else {
                    0.0
                }
            }
            HostParamKind::Enum => {
                if self.variant_count > 1 {
                    plain / f64::from(self.variant_count - 1)
                } else {
                    0.0
                }
            }
            HostParamKind::Float | HostParamKind::Int => {
                let span = self.max - self.min;
                if span.abs() > f64::EPSILON {
                    (plain - self.min) / span
                } else {
                    0.0
                }
            }
        };
        normalized.clamp(0.0, 1.0)
    }
}

/// The host-side write routing for a plugin's parameters: a `key` → [`ParamRoute`]
/// lookup the editor's edit replay resolves each edit through. Meters are absent
/// because they are read-only, so a write addressed to a meter key never resolves
/// and is skipped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditRouting {
    routes: Vec<ParamRoute>,
}

impl EditRouting {
    /// Builds a routing from per-parameter routes, in declaration order.
    #[must_use]
    pub fn new(routes: Vec<ParamRoute>) -> Self {
        Self { routes }
    }

    /// Resolves a parameter key to its route, or `None` for an unknown (or meter)
    /// key. Linear over the parameter list, which is small.
    #[must_use]
    pub fn route(&self, key: &str) -> Option<&ParamRoute> {
        self.routes.iter().find(|route| route.key == key)
    }
}

/// Structured script value.
#[derive(Clone, Debug, PartialEq)]
pub enum StructuredValue {
    /// Null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Numeric value.
    Number(f64),
    /// String value.
    String(String),
}

/// Dependency value made available while evaluating a framework dynamic binding expression.
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicExpressionValue {
    /// Null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Numeric value.
    Number(f64),
    /// String value.
    String(String),
    /// Ordered array value.
    Array(Vec<DynamicExpressionValue>),
    /// Object value keyed by stable property name.
    Object(BTreeMap<String, DynamicExpressionValue>),
}

impl DynamicExpressionValue {
    /// Creates a null value.
    #[must_use]
    pub const fn null() -> Self {
        Self::Null
    }

    /// Creates a boolean value.
    #[must_use]
    pub const fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// Creates a numeric value.
    #[must_use]
    pub const fn number(value: f64) -> Self {
        Self::Number(value)
    }

    /// Creates a string value.
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Creates an array value.
    #[must_use]
    pub fn array(values: impl IntoIterator<Item = DynamicExpressionValue>) -> Self {
        Self::Array(values.into_iter().collect())
    }

    /// Creates an object value.
    #[must_use]
    pub fn object<K>(entries: impl IntoIterator<Item = (K, DynamicExpressionValue)>) -> Self
    where
        K: Into<String>,
    {
        Self::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }

    const fn is_container(&self) -> bool {
        matches!(self, Self::Array(_) | Self::Object(_))
    }
}

/// How one dependency is projected into a dynamic expression evaluation scope.
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicExpressionBinding {
    /// Plain value binding, used by React, Svelte, and Vue-style expressions such as `label`.
    Value(DynamicExpressionValue),
    /// Getter function binding, used by Solid-style signal expressions such as `label()`.
    Getter(DynamicExpressionValue),
}

/// Dependency environment for one framework dynamic binding expression.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DynamicExpressionEnvironment {
    bindings: BTreeMap<String, DynamicExpressionBinding>,
}

impl DynamicExpressionEnvironment {
    /// Creates an empty dynamic expression environment.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
        }
    }

    /// Adds a plain dependency value.
    #[must_use]
    pub fn with_value(mut self, name: impl Into<String>, value: DynamicExpressionValue) -> Self {
        self.bindings
            .insert(name.into(), DynamicExpressionBinding::Value(value));
        self
    }

    /// Adds a getter dependency value.
    #[must_use]
    pub fn with_getter(mut self, name: impl Into<String>, value: DynamicExpressionValue) -> Self {
        self.bindings
            .insert(name.into(), DynamicExpressionBinding::Getter(value));
        self
    }
}

/// Script execution output.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptExecution {
    module_id: String,
    value: StructuredValue,
}

impl ScriptExecution {
    /// Returns the execution value.
    #[must_use]
    pub const fn value(&self) -> &StructuredValue {
        &self.value
    }
}

/// Default maximum accepted source byte length (1 MiB).
const DEFAULT_MAX_SOURCE_BYTES: usize = 1_048_576;

/// Default maximum accepted compiled `JavaScript` byte length (4 MiB).
const DEFAULT_MAX_COMPILED_SOURCE_BYTES: usize = 4_194_304;

/// Default maximum loop iterations permitted before untrusted execution is aborted.
///
/// `boa` leaves its loop-iteration limit at [`u64::MAX`] (unbounded) by default, so an
/// infinite or pathological loop in untrusted script would run forever on the calling thread
/// and wedge the host (or the DAW hosting a plugin editor). This bound makes such a loop
/// terminate with a recoverable error instead.
const DEFAULT_MAX_LOOP_ITERATIONS: u64 = 10_000_000;

/// Default maximum source nesting depth permitted before parsing.
///
/// `JavaScript`/`TypeScript` are parsed by unguarded recursive descent, so deeply nested
/// source can overflow the native stack *before* any runtime limit applies — an uncatchable
/// process abort. Source is depth-bounded before it reaches either parser. Mirrors
/// `hawk2ui_a11y`'s `A11Y_MAX_TREE_DEPTH`.
const DEFAULT_MAX_NESTING_DEPTH: usize = 256;

/// Resource limits enforced on untrusted script source and execution.
///
/// Byte-length limits bound parser/codegen workload; the loop-iteration limit bounds runtime
/// CPU and the nesting-depth limit bounds parse-time native stack usage, so that untrusted
/// script cannot hang, exhaust memory, or crash the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScriptExecutionLimits {
    source_bytes: usize,
    compiled_source_bytes: usize,
    loop_iterations: u64,
    nesting_depth: usize,
}

impl ScriptExecutionLimits {
    /// Default limits: 1 MiB source, 4 MiB compiled, 10,000,000 loop iterations, depth 256.
    pub const DEFAULT: Self = Self {
        source_bytes: DEFAULT_MAX_SOURCE_BYTES,
        compiled_source_bytes: DEFAULT_MAX_COMPILED_SOURCE_BYTES,
        loop_iterations: DEFAULT_MAX_LOOP_ITERATIONS,
        nesting_depth: DEFAULT_MAX_NESTING_DEPTH,
    };

    /// Creates source-size limits for original and compiled `JavaScript`.
    ///
    /// Runtime limits (loop iterations, nesting depth) take their default values; override
    /// them with [`Self::with_max_loop_iterations`] / [`Self::with_max_nesting_depth`].
    #[must_use]
    pub const fn new(max_source_bytes: usize, max_compiled_source_bytes: usize) -> Self {
        Self {
            source_bytes: max_source_bytes,
            compiled_source_bytes: max_compiled_source_bytes,
            loop_iterations: DEFAULT_MAX_LOOP_ITERATIONS,
            nesting_depth: DEFAULT_MAX_NESTING_DEPTH,
        }
    }

    /// Overrides the maximum loop iterations permitted during execution.
    #[must_use]
    pub const fn with_max_loop_iterations(mut self, max_loop_iterations: u64) -> Self {
        self.loop_iterations = max_loop_iterations;
        self
    }

    /// Overrides the maximum source nesting depth permitted before parsing.
    #[must_use]
    pub const fn with_max_nesting_depth(mut self, max_nesting_depth: usize) -> Self {
        self.nesting_depth = max_nesting_depth;
        self
    }

    /// Returns the maximum accepted source byte length.
    #[must_use]
    pub const fn max_source_bytes(&self) -> usize {
        self.source_bytes
    }

    /// Returns the maximum accepted compiled `JavaScript` byte length.
    #[must_use]
    pub const fn max_compiled_source_bytes(&self) -> usize {
        self.compiled_source_bytes
    }

    /// Returns the maximum loop iterations permitted during execution.
    #[must_use]
    pub const fn max_loop_iterations(&self) -> u64 {
        self.loop_iterations
    }

    /// Returns the maximum source nesting depth permitted before parsing.
    #[must_use]
    pub const fn max_nesting_depth(&self) -> usize {
        self.nesting_depth
    }
}

impl Default for ScriptExecutionLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Host call policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostCallPolicy {
    allowed_bindings: BTreeSet<String>,
}

impl HostCallPolicy {
    /// Denies all host calls.
    #[must_use]
    pub const fn deny_all() -> Self {
        Self {
            allowed_bindings: BTreeSet::new(),
        }
    }

    /// Allows the provided host bindings.
    #[must_use]
    pub fn allow<const N: usize>(bindings: [&str; N]) -> Self {
        Self {
            allowed_bindings: bindings.into_iter().map(str::to_string).collect(),
        }
    }

    fn permits(&self, binding: &str) -> bool {
        self.allowed_bindings.contains(binding)
    }
}

/// Timer execution policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerPolicy {
    deterministic: bool,
}

impl TimerPolicy {
    /// Creates deterministic timers for tests and plugin-safe scheduling.
    #[must_use]
    pub const fn deterministic() -> Self {
        Self {
            deterministic: true,
        }
    }
}

/// Promise identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PromiseId(u64);

/// Timer identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TimerId(u64);

/// Promise state.
#[derive(Clone, Debug, PartialEq)]
pub struct PromiseState {
    label: String,
    value: Option<StructuredValue>,
}

impl PromiseState {
    /// Returns resolved value.
    #[must_use]
    pub const fn value(&self) -> Option<&StructuredValue> {
        self.value.as_ref()
    }
}

/// Timer record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimerRecord {
    id: TimerId,
    label: String,
    delay_ms: u64,
}

impl TimerRecord {
    /// Returns timer ID.
    #[must_use]
    pub const fn id(&self) -> TimerId {
        self.id
    }
}

/// Host call record.
#[derive(Clone, Debug, PartialEq)]
pub struct HostCall {
    binding: String,
    payload: StructuredValue,
}

impl HostCall {
    /// Returns called binding name.
    #[must_use]
    pub fn binding(&self) -> &str {
        &self.binding
    }
}

/// Script diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptDiagnostic {
    rule: String,
    message: String,
}

impl ScriptDiagnostic {
    /// Creates a script diagnostic.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Script backend error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptBackendError {
    diagnostic: ScriptDiagnostic,
}

impl ScriptBackendError {
    /// Creates a script backend error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: ScriptDiagnostic::new(rule, message),
        }
    }

    /// Returns structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &ScriptDiagnostic {
        &self.diagnostic
    }

    /// Returns the diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        self.diagnostic.rule()
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        self.diagnostic.message()
    }
}

impl From<ScriptBackendError> for Diagnostic {
    fn from(error: ScriptBackendError) -> Self {
        Self::error(error.diagnostic.rule, error.diagnostic.message)
    }
}

/// Production script backend boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptBackend {
    host_policy: HostCallPolicy,
    timer_policy: TimerPolicy,
    executed_modules: Vec<ScriptModule>,
    promises: BTreeMap<PromiseId, PromiseState>,
    timers: Vec<TimerRecord>,
    host_calls: Vec<HostCall>,
    execution_limits: ScriptExecutionLimits,
    next_promise_id: u64,
    next_timer_id: u64,
    interrupted: Option<String>,
    torn_down: bool,
}

impl ScriptBackend {
    /// Creates a script backend.
    #[must_use]
    pub const fn new(host_policy: HostCallPolicy, timer_policy: TimerPolicy) -> Self {
        Self {
            host_policy,
            timer_policy,
            executed_modules: Vec::new(),
            promises: BTreeMap::new(),
            timers: Vec::new(),
            host_calls: Vec::new(),
            execution_limits: ScriptExecutionLimits::DEFAULT,
            next_promise_id: 1,
            next_timer_id: 1,
            interrupted: None,
            torn_down: false,
        }
    }

    /// Overrides deterministic execution limits.
    #[must_use]
    pub const fn with_execution_limits(mut self, execution_limits: ScriptExecutionLimits) -> Self {
        self.execution_limits = execution_limits;
        self
    }

    /// Executes a module.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when execution is interrupted, torn down, or unsupported.
    pub fn execute_module(
        &mut self,
        module: ScriptModule,
    ) -> Result<ScriptExecution, ScriptBackendError> {
        self.ensure_running()?;
        let executable = Self::compile_module_source(&module, self.execution_limits)?;
        let value = evaluate_javascript(&executable, self.execution_limits)?;
        let execution = ScriptExecution {
            module_id: module.id.clone(),
            value,
        };
        self.executed_modules.push(module);
        Ok(execution)
    }

    /// Executes a module after projecting Rust-owned host promises and timers into Boa.
    ///
    /// The module can call `hawk2ui.promise(label)` to receive a real JavaScript `Promise` backed
    /// by resolved host promise records, and `hawk2ui.onTimer(label, callback)` to register a
    /// deterministic timer callback. After evaluation, Boa jobs are drained, registered timer
    /// callbacks for scheduled Rust timers are invoked, and jobs are drained again.
    ///
    /// The returned value is `globalThis.__hawk2uiResult` after host jobs settle when that global is
    /// defined; otherwise the module evaluation result is returned.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when execution is interrupted, torn down, exceeds configured
    /// limits, fails JavaScript evaluation, or a projected host job fails.
    pub fn execute_module_with_host_jobs(
        &mut self,
        module: ScriptModule,
    ) -> Result<ScriptExecution, ScriptBackendError> {
        self.ensure_running()?;
        let executable = Self::compile_module_source(&module, self.execution_limits)?;
        let value = evaluate_javascript_with_host_jobs(
            &executable,
            self.execution_limits,
            &self.promises,
            &self.timers,
        )?;
        let execution = ScriptExecution {
            module_id: module.id.clone(),
            value,
        };
        self.executed_modules.push(module);
        Ok(execution)
    }

    /// Evaluates one framework dynamic binding expression against a dependency environment.
    ///
    /// The expression is executed by Boa under the backend's deterministic execution limits. Plain
    /// dependencies are projected as `const name = value`; getter dependencies are projected as
    /// `const name = () => value`, matching Solid-style signal reads.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when the backend is stopped, dependency names are unsafe,
    /// dependency values cannot be represented as JavaScript literals, or JavaScript evaluation
    /// fails.
    pub fn evaluate_dynamic_expression(
        &mut self,
        expression: &str,
        environment: &DynamicExpressionEnvironment,
    ) -> Result<StructuredValue, ScriptBackendError> {
        self.ensure_running()?;
        evaluate_dynamic_expression(expression, environment, self.execution_limits)
    }

    /// Evaluates one framework dynamic binding into a native property value.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when expression evaluation fails or the expression result
    /// cannot be represented as a native scalar property value.
    pub fn evaluate_dynamic_binding(
        &mut self,
        binding: &FrameworkDynamicBinding,
        environment: &DynamicExpressionEnvironment,
    ) -> Result<PropValue, ScriptBackendError> {
        structured_value_to_prop_value(
            self.evaluate_dynamic_expression(binding.expression(), environment)?,
        )
    }

    /// Evaluates and applies every dynamic binding carried by a runtime bridge artifact.
    ///
    /// Bindings are applied in compiler declaration order. Each successful application returns a new
    /// runtime tree with the affected node invalidated.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when expression evaluation fails, a value cannot be converted
    /// into a native property value, or the runtime bridge rejects the patch target.
    pub fn apply_dynamic_bindings(
        &mut self,
        mut artifact: NativeRuntimeBridgeArtifact,
        environment: &DynamicExpressionEnvironment,
    ) -> Result<NativeRuntimeBridgeArtifact, ScriptBackendError> {
        let bindings = artifact.dynamic_bindings().to_vec();
        for binding in bindings {
            let value = self.evaluate_dynamic_binding(&binding, environment)?;
            artifact = artifact
                .apply_dynamic_binding(&binding, value)
                .map_err(|error| script_error_from_runtime_bridge(&error))?;
        }
        Ok(artifact)
    }

    /// Compiles a JavaScript or TypeScript module into executable JavaScript without evaluating it.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when source limits are exceeded or TypeScript parsing,
    /// semantic analysis, or transformation fails.
    pub fn compile_module_source(
        module: &ScriptModule,
        execution_limits: ScriptExecutionLimits,
    ) -> Result<String, ScriptBackendError> {
        enforce_source_limit(
            "script.source.too-large",
            "script source exceeds configured execution limit",
            module.source.len(),
            execution_limits.max_source_bytes(),
        )?;
        let executable = match module.kind {
            ScriptModuleKind::JavaScript => module.source.clone(),
            ScriptModuleKind::TypeScript => compile_typescript(
                &module.id,
                &module.source,
                execution_limits.max_nesting_depth(),
            )?,
        };
        enforce_source_limit(
            "script.compiled-source.too-large",
            "compiled JavaScript exceeds configured execution limit",
            executable.len(),
            execution_limits.max_compiled_source_bytes(),
        )?;
        Ok(executable)
    }

    /// Returns executed modules.
    #[must_use]
    pub fn executed_modules(&self) -> &[ScriptModule] {
        &self.executed_modules
    }

    /// Creates a promise record.
    pub fn create_promise(&mut self, label: impl Into<String>) -> PromiseId {
        let id = PromiseId(self.next_promise_id);
        self.next_promise_id = self.next_promise_id.saturating_add(1);
        self.promises.insert(
            id,
            PromiseState {
                label: label.into(),
                value: None,
            },
        );
        id
    }

    /// Resolves a promise.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when the promise is unknown.
    pub fn resolve_promise(
        &mut self,
        id: PromiseId,
        value: StructuredValue,
    ) -> Result<(), ScriptBackendError> {
        let Some(promise) = self.promises.get_mut(&id) else {
            return Err(ScriptBackendError::new(
                "script.promise.missing",
                "promise does not exist",
            ));
        };
        promise.value = Some(value);
        Ok(())
    }

    /// Returns promise state.
    #[must_use]
    pub fn promise_state(&self, id: PromiseId) -> Option<&PromiseState> {
        self.promises.get(&id)
    }

    /// Schedules a deterministic timer.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when timers are unavailable or runtime is torn down.
    pub fn schedule_timer(
        &mut self,
        label: impl Into<String>,
        delay_ms: u64,
    ) -> Result<TimerId, ScriptBackendError> {
        self.ensure_running()?;
        if !self.timer_policy.deterministic {
            return Err(ScriptBackendError::new(
                "script.timer.unavailable",
                "timer policy does not allow scheduling",
            ));
        }
        let id = TimerId(self.next_timer_id);
        self.next_timer_id = self.next_timer_id.saturating_add(1);
        self.timers.push(TimerRecord {
            id,
            label: label.into(),
            delay_ms,
        });
        Ok(id)
    }

    /// Returns timers.
    #[must_use]
    pub fn timers(&self) -> &[TimerRecord] {
        &self.timers
    }

    /// Calls a typed host binding.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when the binding is denied.
    pub fn call_host(
        &mut self,
        binding: impl Into<String>,
        payload: StructuredValue,
    ) -> Result<HostCall, ScriptBackendError> {
        self.ensure_running()?;
        let binding = binding.into();
        if !self.host_policy.permits(&binding) {
            return Err(ScriptBackendError::new(
                "script.host-call.denied",
                "host binding is not permitted by policy",
            ));
        }
        let call = HostCall { binding, payload };
        self.host_calls.push(call.clone());
        Ok(call)
    }

    /// Interrupts further script execution.
    pub fn interrupt(&mut self, reason: impl Into<String>) {
        self.interrupted = Some(reason.into());
    }

    /// Tears down runtime-owned state.
    pub fn teardown(&mut self) {
        self.torn_down = true;
        self.promises.clear();
        self.timers.clear();
    }

    /// Returns whether the backend is torn down.
    #[must_use]
    pub const fn torn_down(&self) -> bool {
        self.torn_down
    }

    fn ensure_running(&self) -> Result<(), ScriptBackendError> {
        if self.torn_down {
            Err(ScriptBackendError::new(
                "script.torn-down",
                "script runtime has been torn down",
            ))
        } else if self.interrupted.is_some() {
            Err(ScriptBackendError::new(
                "script.interrupted",
                "script runtime has been interrupted",
            ))
        } else {
            Ok(())
        }
    }
}

fn enforce_source_limit(
    rule: &'static str,
    message: &'static str,
    actual_bytes: usize,
    max_bytes: usize,
) -> Result<(), ScriptBackendError> {
    if actual_bytes <= max_bytes {
        return Ok(());
    }
    Err(ScriptBackendError::new(
        rule,
        format!("{message}: {actual_bytes} bytes exceeds {max_bytes} bytes"),
    ))
}

/// Rejects source whose bracket nesting depth exceeds `max_depth`.
///
/// `JavaScript`/`TypeScript` are parsed by unguarded recursive descent (neither `oxc_parser`
/// nor `boa`'s parser bounds nesting depth), so deeply nested source can overflow the native
/// thread stack *during parsing* — a `SIGSEGV`/abort that [`std::panic::catch_unwind`] cannot
/// recover. This bound therefore runs before either parser sees the source.
///
/// The scan counts `(`, `[`, `{` as openers and their closers, saturating at zero so leading
/// closers cannot mask later nesting. It tracks depth (not raw count), so balanced brackets in
/// string literals do not trip it, and because every opener counts the bound cannot be bypassed.
fn enforce_nesting_depth(source: &str, max_depth: usize) -> Result<(), ScriptBackendError> {
    let mut depth: usize = 0;
    for &byte in source.as_bytes() {
        match byte {
            b'(' | b'[' | b'{' => {
                depth += 1;
                if depth > max_depth {
                    return Err(ScriptBackendError::new(
                        "script.source.too-deeply-nested",
                        format!(
                            "script source nesting depth exceeds configured limit of {max_depth}"
                        ),
                    ));
                }
            }
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

/// Applies runtime resource limits to a freshly created context before untrusted execution.
///
/// `boa` leaves the loop-iteration limit unbounded ([`u64::MAX`]) by default; bounding it makes
/// an infinite or pathological loop terminate with a recoverable error instead of hanging the
/// calling thread. `boa`'s recursion and stack-size limits are already bounded by its own
/// defaults.
fn apply_runtime_limits(context: &mut Context, limits: ScriptExecutionLimits) {
    context
        .runtime_limits_mut()
        .set_loop_iteration_limit(limits.max_loop_iterations());
}

/// Stack size for the worker thread that parses and evaluates untrusted source.
///
/// Untrusted parsing runs on a dedicated thread with this fixed, generous stack so the
/// nesting-depth bound is calibrated against a *known* stack rather than whatever (possibly
/// small) stack the host or DAW happens to invoke us on. The worker does not *contain* a stack
/// overflow — that remains prevented by [`enforce_nesting_depth`] — but it decouples the safe
/// parse depth from the caller's thread, which matters for the embedded plugin editor.
const SCRIPT_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Runs untrusted parsing/evaluation on a dedicated worker thread, returning its result.
///
/// The worker has a known, generous stack ([`SCRIPT_WORKER_STACK_BYTES`]) so legitimately nested
/// source parses regardless of the caller's stack size. Joining the worker also converts a
/// catchable `boa`/`oxc` panic into a diagnostic instead of letting it unwind through the host. A
/// native stack overflow is not a catchable panic — that case is prevented up front by
/// [`enforce_nesting_depth`].
fn run_on_worker<T: Send>(
    panic_rule: &'static str,
    operation: impl FnOnce() -> Result<T, ScriptBackendError> + Send,
) -> Result<T, ScriptBackendError> {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .stack_size(SCRIPT_WORKER_STACK_BYTES)
            .spawn_scoped(scope, operation)
            .map_err(|error| {
                ScriptBackendError::new(
                    "script.worker.spawn-failed",
                    format!("failed to spawn script worker thread: {error}"),
                )
            })?;
        worker.join().map_err(|_| {
            ScriptBackendError::new(
                panic_rule,
                "script engine panicked while processing untrusted source",
            )
        })?
    })
}

fn compile_typescript(
    module_id: &str,
    source: &str,
    max_nesting_depth: usize,
) -> Result<String, ScriptBackendError> {
    enforce_nesting_depth(source, max_nesting_depth)?;
    run_on_worker("script.typescript.panicked", || {
        compile_typescript_inner(module_id, source)
    })
}

fn compile_typescript_inner(module_id: &str, source: &str) -> Result<String, ScriptBackendError> {
    let allocator = Allocator::default();
    let source_path = Path::new(module_id);
    let source_type = SourceType::from_path(source_path).unwrap_or_else(|_| SourceType::ts());
    let parse_return = Parser::new(&allocator, source, source_type).parse();
    if !parse_return.errors.is_empty() {
        return Err(ScriptBackendError::new(
            "script.typescript.parse-failed",
            format_oxc_diagnostics("TypeScript parse failed", parse_return.errors),
        ));
    }

    let mut program = parse_return.program;
    let semantic_return = SemanticBuilder::new()
        .with_excess_capacity(2.0)
        .with_enum_eval(true)
        .build(&program);
    if !semantic_return.errors.is_empty() {
        return Err(ScriptBackendError::new(
            "script.typescript.semantic-failed",
            format_oxc_diagnostics(
                "TypeScript semantic analysis failed",
                semantic_return.errors,
            ),
        ));
    }

    let mut options = TransformOptions::default();
    options.helper_loader.mode = HelperLoaderMode::External;
    let transform_return = Transformer::new(&allocator, source_path, &options)
        .build_with_scoping(semantic_return.semantic.into_scoping(), &mut program);
    if !transform_return.errors.is_empty() {
        return Err(ScriptBackendError::new(
            "script.typescript.transform-failed",
            format_oxc_diagnostics("TypeScript transform failed", transform_return.errors),
        ));
    }

    Ok(Codegen::new().build(&program).code)
}

fn format_oxc_diagnostics<T: fmt::Debug>(prefix: &'static str, errors: Vec<T>) -> String {
    let details = errors
        .into_iter()
        .map(|error| format!("{error:?}"))
        .collect::<Vec<_>>()
        .join("; ");
    format!("{prefix}: {details}")
}

fn evaluate_javascript(
    source: &str,
    limits: ScriptExecutionLimits,
) -> Result<StructuredValue, ScriptBackendError> {
    enforce_nesting_depth(source, limits.max_nesting_depth())?;
    run_on_worker("script.eval.panicked", || {
        let mut context = Context::default();
        apply_runtime_limits(&mut context, limits);
        let value = context.eval(Source::from_bytes(source)).map_err(|error| {
            ScriptBackendError::new(
                "script.eval.failed",
                format!("JavaScript execution failed: {error}"),
            )
        })?;
        context.run_jobs().map_err(|error| {
            ScriptBackendError::new(
                "script.jobs.failed",
                format!("JavaScript job queue failed: {error}"),
            )
        })?;
        structured_value_from_js(&value)
    })
}

fn evaluate_dynamic_expression(
    expression: &str,
    environment: &DynamicExpressionEnvironment,
    limits: ScriptExecutionLimits,
) -> Result<StructuredValue, ScriptBackendError> {
    if expression.trim().is_empty() {
        return Err(ScriptBackendError::new(
            "script.dynamic-expression.empty",
            "dynamic binding expression must not be empty",
        ));
    }
    let source = dynamic_expression_source(expression, environment)?;
    enforce_source_limit(
        "script.dynamic-expression.too-large",
        "dynamic expression source exceeds configured execution limit",
        source.len(),
        limits.max_source_bytes(),
    )?;
    evaluate_javascript(&source, limits)
}

fn dynamic_expression_source(
    expression: &str,
    environment: &DynamicExpressionEnvironment,
) -> Result<String, ScriptBackendError> {
    let mut source = String::from("\"use strict\";\n");
    for (name, binding) in &environment.bindings {
        if !is_safe_identifier(name) {
            return Err(ScriptBackendError::new(
                "script.dynamic-expression.dependency-invalid",
                format!(
                    "dynamic expression dependency `{name}` is not a safe JavaScript identifier"
                ),
            ));
        }
        match binding {
            DynamicExpressionBinding::Value(value) => {
                writeln!(
                    source,
                    "const {name} = {};",
                    dynamic_expression_binding_literal(value)?
                )
                .map_err(dynamic_expression_source_error)?;
            }
            DynamicExpressionBinding::Getter(value) => {
                writeln!(
                    source,
                    "const {name} = () => {};",
                    dynamic_expression_binding_literal(value)?
                )
                .map_err(dynamic_expression_source_error)?;
            }
        }
    }
    write!(source, "({});", expression.trim()).map_err(dynamic_expression_source_error)?;
    Ok(source)
}

fn dynamic_expression_source_error(error: fmt::Error) -> ScriptBackendError {
    ScriptBackendError::new(
        "script.dynamic-expression.source-failed",
        format!("failed to build dynamic expression source: {error}"),
    )
}

fn dynamic_expression_binding_literal(
    value: &DynamicExpressionValue,
) -> Result<String, ScriptBackendError> {
    let literal = dynamic_expression_value_js_literal(value)?;
    if value.is_container() {
        Ok(format!("Object.freeze({literal})"))
    } else {
        Ok(literal)
    }
}

fn dynamic_expression_value_js_literal(
    value: &DynamicExpressionValue,
) -> Result<String, ScriptBackendError> {
    match value {
        DynamicExpressionValue::Null => Ok("null".to_string()),
        DynamicExpressionValue::Bool(value) => Ok(value.to_string()),
        DynamicExpressionValue::Number(value) if value.is_finite() => Ok(value.to_string()),
        DynamicExpressionValue::Number(_) => Err(ScriptBackendError::new(
            "script.dynamic-expression.value-invalid",
            "dynamic expression numeric dependency value must be finite",
        )),
        DynamicExpressionValue::String(value) => Ok(js_string_literal(value)),
        DynamicExpressionValue::Array(values) => {
            let mut literal = String::from("[");
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    literal.push(',');
                }
                literal.push_str(&dynamic_expression_value_js_literal(value)?);
            }
            literal.push(']');
            Ok(literal)
        }
        DynamicExpressionValue::Object(values) => {
            let mut literal = String::from("{");
            for (index, (key, value)) in values.iter().enumerate() {
                if index > 0 {
                    literal.push(',');
                }
                literal.push_str(&js_string_literal(key));
                literal.push(':');
                literal.push_str(&dynamic_expression_value_js_literal(value)?);
            }
            literal.push('}');
            Ok(literal)
        }
    }
}

fn is_safe_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|character| character == '_' || character == '$' || character.is_ascii_alphanumeric())
}

fn structured_value_to_prop_value(value: StructuredValue) -> Result<PropValue, ScriptBackendError> {
    match value {
        StructuredValue::Bool(value) => Ok(PropValue::Bool(value)),
        StructuredValue::Number(value) if value.is_finite() => Ok(PropValue::Number(value)),
        StructuredValue::Number(_) => Err(ScriptBackendError::new(
            "script.dynamic-binding.value-invalid",
            "dynamic binding numeric result must be finite",
        )),
        StructuredValue::String(value) => Ok(PropValue::String(value)),
        StructuredValue::Null => Err(ScriptBackendError::new(
            "script.dynamic-binding.value-unsupported",
            "dynamic binding expression result cannot be null or undefined",
        )),
    }
}

fn script_error_from_runtime_bridge(error: &NativeRuntimeBridgeError) -> ScriptBackendError {
    ScriptBackendError::new(error.rule(), error.message())
}

fn evaluate_javascript_with_host_jobs(
    source: &str,
    limits: ScriptExecutionLimits,
    promises: &BTreeMap<PromiseId, PromiseState>,
    timers: &[TimerRecord],
) -> Result<StructuredValue, ScriptBackendError> {
    enforce_nesting_depth(source, limits.max_nesting_depth())?;
    run_on_worker("script.host-jobs.panicked", || {
        evaluate_javascript_with_host_jobs_inner(source, limits, promises, timers)
    })
}

fn evaluate_javascript_with_host_jobs_inner(
    source: &str,
    limits: ScriptExecutionLimits,
    promises: &BTreeMap<PromiseId, PromiseState>,
    timers: &[TimerRecord],
) -> Result<StructuredValue, ScriptBackendError> {
    let mut context = Context::default();
    apply_runtime_limits(&mut context, limits);
    eval_js_unit(
        &mut context,
        host_job_prelude(),
        "script.host-jobs.bootstrap-failed",
    )?;
    for promise in promises.values() {
        if let Some(value) = promise.value() {
            eval_js_unit(
                &mut context,
                &format!(
                    "globalThis.__hawk2uiResolve({}, {});",
                    js_string_literal(&promise.label),
                    structured_value_js_literal(value)?
                ),
                "script.host-jobs.promise-bootstrap-failed",
            )?;
        }
    }

    let evaluation_result = context.eval(Source::from_bytes(source)).map_err(|error| {
        ScriptBackendError::new(
            "script.eval.failed",
            format!("JavaScript execution failed: {error}"),
        )
    })?;
    run_boa_jobs(&mut context)?;

    for timer in timers {
        eval_js_unit(
            &mut context,
            &format!(
                "globalThis.__hawk2uiFlushTimer({});",
                js_string_literal(&timer.label)
            ),
            "script.host-jobs.timer-failed",
        )?;
        run_boa_jobs(&mut context)?;
    }

    let settled_result = context
        .eval(Source::from_bytes(
            "typeof globalThis.__hawk2uiResult === 'undefined' ? undefined : globalThis.__hawk2uiResult",
        ))
        .map_err(|error| {
            ScriptBackendError::new(
                "script.host-jobs.result-read-failed",
                format!("JavaScript host job result read failed: {error}"),
            )
        })?;
    if matches!(settled_result.variant(), JsVariant::Undefined) {
        structured_value_from_js(&evaluation_result)
    } else {
        structured_value_from_js(&settled_result)
    }
}

fn host_job_prelude() -> &'static str {
    r#"
const __hawk2uiResolvedPromises = new Map();
const __hawk2uiTimerCallbacks = new Map();
globalThis.__hawk2uiResolve = (label, value) => {
  __hawk2uiResolvedPromises.set(label, value);
};
globalThis.__hawk2uiFlushTimer = (label) => {
  const callback = __hawk2uiTimerCallbacks.get(label);
  if (callback !== undefined) {
    callback();
  }
};
globalThis.hawk2ui = Object.freeze({
  promise(label) {
    if (!__hawk2uiResolvedPromises.has(label)) {
      return Promise.reject(new Error(`host promise is not resolved: ${label}`));
    }
    return Promise.resolve(__hawk2uiResolvedPromises.get(label));
  },
  onTimer(label, callback) {
    if (typeof callback !== "function") {
      throw new TypeError("timer callback must be a function");
    }
    __hawk2uiTimerCallbacks.set(label, callback);
    return label;
  }
});
"#
}

fn eval_js_unit(
    context: &mut Context,
    source: &str,
    rule: &'static str,
) -> Result<(), ScriptBackendError> {
    context.eval(Source::from_bytes(source)).map_err(|error| {
        ScriptBackendError::new(rule, format!("JavaScript host job setup failed: {error}"))
    })?;
    Ok(())
}

fn run_boa_jobs(context: &mut Context) -> Result<(), ScriptBackendError> {
    context.run_jobs().map_err(|error| {
        ScriptBackendError::new(
            "script.jobs.failed",
            format!("JavaScript job queue failed: {error}"),
        )
    })
}

fn structured_value_js_literal(value: &StructuredValue) -> Result<String, ScriptBackendError> {
    match value {
        StructuredValue::Null => Ok("null".to_string()),
        StructuredValue::Bool(value) => Ok(value.to_string()),
        StructuredValue::Number(value) if value.is_finite() => Ok(value.to_string()),
        StructuredValue::Number(_) => Err(ScriptBackendError::new(
            "script.value.invalid-number",
            "host promise numeric value must be finite",
        )),
        StructuredValue::String(value) => Ok(js_string_literal(value)),
    }
}

fn js_string_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", u32::from(character));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn structured_value_from_js(value: &JsValue) -> Result<StructuredValue, ScriptBackendError> {
    match value.variant() {
        JsVariant::Null | JsVariant::Undefined => Ok(StructuredValue::Null),
        JsVariant::Boolean(value) => Ok(StructuredValue::Bool(value)),
        JsVariant::Float64(value) => Ok(StructuredValue::Number(value)),
        JsVariant::Integer32(value) => Ok(StructuredValue::Number(f64::from(value))),
        JsVariant::String(value) => {
            value
                .to_std_string()
                .map(StructuredValue::String)
                .map_err(|_| {
                    ScriptBackendError::new(
                        "script.value.unsupported-string",
                        "JavaScript string result cannot be represented as UTF-8",
                    )
                })
        }
        JsVariant::BigInt(_) | JsVariant::Object(_) | JsVariant::Symbol(_) => {
            Err(ScriptBackendError::new(
                "script.value.unsupported",
                "JavaScript result type cannot be represented as a structured Hawk2UI value",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-script");
    }

    #[test]
    fn entry_mount_bootstrap_wraps_a_mount_function() {
        let bootstrap = entry_mount_bootstrap("export function mount(host) { return \"{}\"; }")
            .expect("a mount function bootstraps");
        // The export is rewritten to a local declaration and the host-invoked
        // result is serialized to a JSON node tree.
        assert!(bootstrap.contains("function mount(host)"));
        assert!(!bootstrap.contains("export function mount"));
        assert!(bootstrap.contains("JSON.stringify(mount(__hawk2ui_host));"));
    }

    #[test]
    fn entry_mount_bootstrap_returns_none_without_a_mount_function() {
        assert!(entry_mount_bootstrap("export function other() {}").is_none());
    }

    #[test]
    fn module_kind_is_inferred_from_source_path() {
        assert_eq!(
            ScriptModule::for_source_path("src/main.ts", "").kind(),
            ScriptModuleKind::TypeScript
        );
        assert_eq!(
            ScriptModule::for_source_path("src/main.js", "").kind(),
            ScriptModuleKind::JavaScript
        );
    }

    #[test]
    fn projects_params_and_meters_into_the_entry_host() {
        let snapshot = HostSnapshot {
            params: vec![
                HostParam {
                    key: "cutoff".into(),
                    id: 3,
                    kind: HostParamKind::Float,
                    value: HostParamValue::Float(1200.0),
                    normalized: 0.42,
                    text: "1.20 kHz".into(),
                    variants: Vec::new(),
                },
                HostParam {
                    key: "bypass".into(),
                    id: 0,
                    kind: HostParamKind::Bool,
                    value: HostParamValue::Bool(true),
                    normalized: 1.0,
                    text: "On".into(),
                    variants: Vec::new(),
                },
            ],
            meters: vec![HostMeter {
                key: "out".into(),
                id: 1 << 24,
                value: 0.5,
            }],
        };
        let source = r#"
export function mount(host) {
    const c = host.param("cutoff");
    return {
        id: "root",
        type: "text",
        text: c.id + "|" + c.kind + "|" + c.value + "|" + c.text + "|" + c.normalized
            + "|" + host.param("bypass").value + "|" + host.meter("out")
            + "|" + host.params.length + "|" + host.meters.length
    };
}
"#;
        let bootstrap = entry_mount_bootstrap_with_host(source, &snapshot, &[], "null")
            .expect("mount is wrapped");
        let mut backend =
            ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
        let execution = backend
            .execute_module(ScriptModule::javascript("editor.js", bootstrap.as_str()))
            .expect("the projected entry script executes");
        let StructuredValue::String(json) = execution.value() else {
            panic!("mount must return a serialized node tree");
        };
        // id 3 (truce ParamId); float kind; plain value 1200; formatted text;
        // normalized; bypass reads as a JS boolean; the meter as its level; two
        // params, one meter.
        assert!(
            json.contains("3|float|1200|1.20 kHz|0.42|true|0.5|2|1"),
            "{json}"
        );
    }

    #[test]
    fn an_unknown_param_or_meter_key_throws() {
        let snapshot = HostSnapshot::default();
        let bootstrap = entry_mount_bootstrap_with_host(
            r#"export function mount(host) { return host.param("nope"); }"#,
            &snapshot,
            &[],
            "null",
        )
        .expect("mount is wrapped");
        let mut backend =
            ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
        assert!(
            backend
                .execute_module(ScriptModule::javascript("editor.js", bootstrap.as_str()))
                .is_err(),
            "reading an unknown parameter key must throw"
        );
    }

    #[test]
    fn dynamic_expression_evaluator_reads_values_objects_and_signal_getters() {
        let mut backend =
            ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
        let environment = DynamicExpressionEnvironment::new()
            .with_value("label", DynamicExpressionValue::string("Live"))
            .with_value(
                "params",
                DynamicExpressionValue::object([(
                    "title",
                    DynamicExpressionValue::string("Filter"),
                )]),
            )
            .with_getter("meter", DynamicExpressionValue::number(0.75));

        let value = backend
            .evaluate_dynamic_expression("label + ':' + params.title + ':' + meter()", &environment)
            .expect("dynamic binding expression evaluates");

        assert_eq!(value, StructuredValue::String("Live:Filter:0.75".into()));
    }

    #[test]
    fn dynamic_expression_evaluator_rejects_unsafe_dependency_names() {
        let mut backend =
            ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
        let environment = DynamicExpressionEnvironment::new().with_value(
            "label; globalThis.pwned = true",
            DynamicExpressionValue::string("bad"),
        );

        let error = backend
            .evaluate_dynamic_expression("label", &environment)
            .expect_err("dependency names must be identifier-safe");

        assert_eq!(error.rule(), "script.dynamic-expression.dependency-invalid");
    }

    #[test]
    fn dynamic_binding_evaluator_applies_values_to_runtime_artifacts() {
        use hawk2ui_authoring::{
            FrameworkDynamicBinding, FrameworkNativeNode, FrameworkNativeProgram,
            NativeRuntimeBridge, PropValue,
        };
        use hawk2ui_runtime::{RuntimeViewId, RuntimeVisual};

        let program = FrameworkNativeProgram::new(
            FrameworkNativeNode::new("root", hawk2ui_authoring::ElementKind::View).with_child(
                "title",
                FrameworkNativeNode::new("title", hawk2ui_authoring::ElementKind::Text)
                    .with_prop("width", PropValue::Number(160.0))
                    .with_prop("height", PropValue::Number(32.0)),
            ),
        )
        .with_dynamic_binding(FrameworkDynamicBinding::prop(
            "title",
            "text",
            "label",
            vec!["label".to_string()],
        ));
        let native = program
            .to_native_authoring_artifact("App.tsx", true)
            .expect("program finalizes");
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact(&native)
            .expect("program bridges");
        let environment = DynamicExpressionEnvironment::new()
            .with_value("label", DynamicExpressionValue::string("Live"));
        let mut backend =
            ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());

        let patched = backend
            .apply_dynamic_bindings(runtime, &environment)
            .expect("dynamic binding evaluates and patches runtime");

        let title = patched
            .runtime_tree()
            .node(&RuntimeViewId::new("title"))
            .expect("title node exists");
        assert!(matches!(
            title.visual(),
            RuntimeVisual::Text(text) if text.text() == "Live"
        ));
    }

    fn run_entry(source: &str, incoming_ui: &str) -> EntryEnvelope {
        run_entry_with_events(source, &[], incoming_ui)
    }

    fn run_entry_with_events(
        source: &str,
        events: &[FrameInput],
        incoming_ui: &str,
    ) -> EntryEnvelope {
        let snapshot = HostSnapshot {
            params: vec![HostParam {
                key: "cutoff".into(),
                id: 3,
                kind: HostParamKind::Float,
                value: HostParamValue::Float(1200.0),
                normalized: 0.42,
                text: "1.20 kHz".into(),
                variants: Vec::new(),
            }],
            meters: Vec::new(),
        };
        let bootstrap = entry_mount_bootstrap_with_host(source, &snapshot, events, incoming_ui)
            .expect("mount wrapped");
        let mut backend =
            ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
        let execution = backend
            .execute_module(ScriptModule::javascript("editor.js", bootstrap.as_str()))
            .expect("the projected entry script executes");
        let StructuredValue::String(json) = execution.value() else {
            panic!("the entry must return a serialized envelope");
        };
        parse_entry_envelope(json).expect("the entry returns a valid envelope")
    }

    #[test]
    fn projects_input_events_into_the_entry_host() {
        // The host.events shape: kind-tagged pointer / key / focus events, in
        // arrival order, with a fractional logical coordinate preserved through
        // the JSON-⊂-JS embed.
        let envelope = run_entry_with_events(
            r#"
export function mount(host) {
    const parts = host.events.map(function (e) {
        if (e.kind === "pointer") return "P:" + e.x + ":" + e.y + ":" + e.button;
        if (e.kind === "key") return "K:" + e.key + ":" + e.pressed;
        if (e.kind === "focus") return "F:" + e.focused;
        return "?";
    });
    return { id: "ev|" + host.events.length + "|" + parts.join("|"), type: "view" };
}
"#,
            &[
                FrameInput::Pointer {
                    x: 12.5,
                    y: 34.0,
                    button: "left-down".into(),
                },
                FrameInput::Key {
                    key: "a".into(),
                    pressed: true,
                },
                FrameInput::Focus { focused: true },
            ],
            "null",
        );
        assert!(
            envelope
                .tree_json
                .contains("ev|3|P:12.5:34:left-down|K:a:true|F:true"),
            "{}",
            envelope.tree_json
        );
    }

    #[test]
    fn an_idle_frame_sees_an_empty_events_array() {
        // D3: a frame with no input gets `[]` — a real, iterable array of length 0.
        let envelope = run_entry_with_events(
            r#"
export function mount(host) {
    return { id: "idle|" + host.events.length + "|" + Array.isArray(host.events), type: "view" };
}
"#,
            &[],
            "null",
        );
        assert!(
            envelope.tree_json.contains("idle|0|true"),
            "{}",
            envelope.tree_json
        );
    }

    #[test]
    fn host_on_dispatches_current_frame_events_to_handlers() {
        let envelope = run_entry_with_events(
            r#"
export function mount(host) {
    const seen = [];
    host.on("pointer", function (ev) {
        seen.push("P:" + ev.button);
        if (ev.button === "left-down") host.setParam("cutoff", 0.66);
    });
    host.on("key", function (ev) { seen.push("K:" + ev.key + ":" + ev.pressed); });
    host.on("focus", function (ev) { seen.push("F:" + ev.focused); });
    host.on("input", function (ev) { seen.push("I:" + ev.kind); });
    return { id: "on|" + seen.join("|"), type: "view" };
}
"#,
            &[
                FrameInput::Pointer {
                    x: 12.5,
                    y: 34.0,
                    button: "left-down".into(),
                },
                FrameInput::Key {
                    key: "a".into(),
                    pressed: true,
                },
                FrameInput::Focus { focused: true },
            ],
            "null",
        );
        assert!(
            envelope
                .tree_json
                .contains("on|P:left-down|K:a:true|F:true|I:pointer|I:key|I:focus"),
            "{}",
            envelope.tree_json
        );
        assert_eq!(
            envelope.edits,
            vec![HostEdit::Set {
                key: "cutoff".into(),
                normalized: 0.66,
            }]
        );
    }

    #[test]
    fn host_on_ignores_unknown_event_names_for_scaffold_compatibility() {
        // Existing authored entries (the `new` scaffold, examples) may register
        // non-input lifecycle names before the lifecycle surface is finalized.
        // Unknown names are ignored rather than treated as fatal input errors.
        let envelope = run_entry(
            r#"
export function mount(host) {
    host.on("ready", function () {});
    return { id: "on-ok", type: "view" };
}
"#,
            "null",
        );
        assert!(
            envelope.tree_json.contains("on-ok"),
            "{}",
            envelope.tree_json
        );
    }

    #[test]
    fn write_verbs_record_an_ordered_edit_list() {
        let envelope = run_entry(
            r#"
export function mount(host) {
    host.beginEdit("cutoff");
    host.setParam("cutoff", 0.55);
    host.setParamPlain("cutoff", 1200);
    host.endEdit("cutoff");
    host.automate("cutoff", 1.0);
    return { id: "root", type: "view" };
}
"#,
            "null",
        );
        assert_eq!(
            envelope.edits,
            vec![
                HostEdit::Begin {
                    key: "cutoff".into()
                },
                HostEdit::Set {
                    key: "cutoff".into(),
                    normalized: 0.55,
                },
                HostEdit::SetPlain {
                    key: "cutoff".into(),
                    plain: 1200.0,
                },
                HostEdit::End {
                    key: "cutoff".into()
                },
                HostEdit::Automate {
                    key: "cutoff".into(),
                    normalized: 1.0,
                },
            ]
        );
        assert!(
            envelope.tree_json.contains("\"root\""),
            "{}",
            envelope.tree_json
        );
    }

    #[test]
    fn ui_state_threads_in_and_out() {
        // The incoming blob carries a drag and a frame counter; the script reads
        // them and writes the carried-forward, advanced blob via setUi.
        let envelope = run_entry(
            r#"
export function mount(host) {
    const dragging = host.ui && host.ui.dragging ? host.ui.dragging : "none";
    const frames = (host.ui ? host.ui.frames : 0) + 1;
    host.setUi({ dragging: dragging, frames: frames });
    return { id: "root", type: "view" };
}
"#,
            r#"{"dragging":"cutoff","frames":2}"#,
        );
        assert!(
            envelope.ui_json.contains("\"dragging\":\"cutoff\""),
            "the drag must thread forward: {}",
            envelope.ui_json
        );
        assert!(
            envelope.ui_json.contains("\"frames\":3"),
            "the frame counter must advance across the threaded blob: {}",
            envelope.ui_json
        );
    }

    #[test]
    fn untouched_ui_defaults_to_the_incoming_blob() {
        let envelope = run_entry(
            r#"export function mount(host) { return { id: "root", type: "view" }; }"#,
            r#"{"dragging":"cutoff"}"#,
        );
        assert!(
            envelope.ui_json.contains("\"dragging\":\"cutoff\""),
            "an entry that never calls setUi must persist the incoming ui: {}",
            envelope.ui_json
        );
    }

    #[test]
    fn an_unknown_write_key_throws() {
        let snapshot = HostSnapshot::default();
        let bootstrap = entry_mount_bootstrap_with_host(
            r#"export function mount(host) { host.setParam("nope", 0.5); return { id: "root", type: "view" }; }"#,
            &snapshot,
            &[],
            "null",
        )
        .expect("mount is wrapped");
        let mut backend =
            ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
        assert!(
            backend
                .execute_module(ScriptModule::javascript("editor.js", bootstrap.as_str()))
                .is_err(),
            "writing an unknown parameter key must throw"
        );
    }

    #[test]
    fn parse_entry_envelope_splits_tree_edits_and_ui() {
        let envelope = parse_entry_envelope(
            r#"{"tree":{"id":"root","type":"view"},"edits":[{"op":"set","key":"gain","normalized":0.25}],"ui":{"dragging":"gain"}}"#,
        )
        .expect("a well-formed envelope parses");
        assert!(envelope.tree_json.contains("\"root\""));
        assert_eq!(
            envelope.edits,
            vec![HostEdit::Set {
                key: "gain".into(),
                normalized: 0.25,
            }]
        );
        assert!(envelope.ui_json.contains("\"dragging\":\"gain\""));
    }

    #[test]
    fn parse_entry_envelope_defaults_absent_edits_and_ui() {
        let envelope = parse_entry_envelope(r#"{"tree":{"id":"root","type":"view"}}"#)
            .expect("tree-only envelope parses");
        assert!(envelope.edits.is_empty());
        assert_eq!(envelope.ui_json, "null");
    }

    #[test]
    fn parse_entry_envelope_rejects_a_non_envelope() {
        assert!(
            parse_entry_envelope("42").is_err(),
            "a bare number is not an envelope"
        );
    }

    #[test]
    fn param_route_normalizes_plain_by_kind() {
        let float = ParamRoute {
            key: "cutoff".into(),
            id: 0,
            kind: HostParamKind::Float,
            min: 20.0,
            max: 20020.0,
            variant_count: 0,
        };
        assert!(
            (float.normalize_plain(10020.0) - 0.5).abs() < 1e-9,
            "midpoint"
        );
        assert!((float.normalize_plain(20.0)).abs() < 1e-9, "min");
        assert!(
            (float.normalize_plain(40020.0) - 1.0).abs() < 1e-9,
            "above max clamps to 1"
        );

        let boolean = ParamRoute {
            key: "bypass".into(),
            id: 1,
            kind: HostParamKind::Bool,
            min: 0.0,
            max: 0.0,
            variant_count: 0,
        };
        assert!((boolean.normalize_plain(1.0) - 1.0).abs() < 1e-9);
        assert!((boolean.normalize_plain(0.0)).abs() < 1e-9);

        let enumerated = ParamRoute {
            key: "mode".into(),
            id: 2,
            kind: HostParamKind::Enum,
            min: 0.0,
            max: 0.0,
            variant_count: 3,
        };
        assert!(
            (enumerated.normalize_plain(0.0)).abs() < 1e-9,
            "first variant"
        );
        assert!(
            (enumerated.normalize_plain(2.0) - 1.0).abs() < 1e-9,
            "last variant"
        );
    }

    #[test]
    fn edit_routing_resolves_by_key_and_misses_unknown() {
        let routing = EditRouting::new(vec![ParamRoute {
            key: "cutoff".into(),
            id: 7,
            kind: HostParamKind::Float,
            min: 0.0,
            max: 1.0,
            variant_count: 0,
        }]);
        assert_eq!(routing.route("cutoff").map(|route| route.id), Some(7));
        assert!(routing.route("nope").is_none());
    }
}
