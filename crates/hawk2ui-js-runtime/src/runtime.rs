use std::{rc::Rc, sync::mpsc, thread, time::Duration};

use deno_core::{JsRuntime, ModuleSpecifier, PollEventLoopOptions, RuntimeOptions, v8};

use crate::JsRuntimeError;
use crate::extensions::capabilities;
use crate::extensions::crypto;
use crate::extensions::scene::{self, SceneCommitLog};
use crate::extensions::timers;
use crate::module_loader::HawkJsModuleGraph;
use crate::permissions::HawkRuntimeCapabilities;
use crate::scene_ops::SceneOpBatch;

/// Embedded JavaScript runtime for `Hawk2UI` application and plugin UI code.
pub struct HawkJsRuntime {
    runtime: JsRuntime,
    scene_commit_log: SceneCommitLog,
    entrypoint_module: Option<ModuleSpecifier>,
}

/// Primitive JavaScript result value that can cross the native runtime boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum JsRuntimeValue {
    /// JavaScript `null` or `undefined`.
    Null,
    /// JavaScript boolean.
    Bool(bool),
    /// JavaScript number.
    Number(f64),
    /// JavaScript string.
    String(String),
}

impl HawkJsRuntime {
    /// Creates a runtime with only the narrow scene bridge registered.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when the runtime cannot remove privileged startup globals.
    pub fn new() -> Result<Self, JsRuntimeError> {
        Self::with_capabilities(HawkRuntimeCapabilities::deny_all())
    }

    /// Creates a runtime with an explicit capability context.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when the runtime cannot remove privileged startup globals.
    pub fn with_capabilities(
        capabilities: HawkRuntimeCapabilities,
    ) -> Result<Self, JsRuntimeError> {
        let scene_commit_log = SceneCommitLog::default();
        let mut runtime = Self {
            runtime: JsRuntime::new(RuntimeOptions {
                extensions: vec![
                    scene::extension(scene_commit_log.clone()),
                    crypto::extension(),
                    timers::extension(),
                    capabilities::extension(capabilities),
                ],
                ..Default::default()
            }),
            scene_commit_log,
            entrypoint_module: None,
        };
        runtime.bootstrap_unprivileged_globals()?;
        Ok(runtime)
    }

    /// Creates a runtime backed by a sealed in-memory module graph.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when module specifiers are invalid or startup bootstrap fails.
    pub fn from_module_graph(module_graph: HawkJsModuleGraph) -> Result<Self, JsRuntimeError> {
        Self::from_module_graph_with_capabilities(module_graph, HawkRuntimeCapabilities::deny_all())
    }

    /// Creates a runtime backed by a sealed in-memory module graph and explicit capabilities.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when module specifiers are invalid or startup bootstrap fails.
    pub fn from_module_graph_with_capabilities(
        module_graph: HawkJsModuleGraph,
        capabilities: HawkRuntimeCapabilities,
    ) -> Result<Self, JsRuntimeError> {
        let entrypoint_module = module_graph.entrypoint_specifier()?;
        let module_loader = module_graph.into_static_loader()?;
        let scene_commit_log = SceneCommitLog::default();
        let mut runtime = Self {
            runtime: JsRuntime::new(RuntimeOptions {
                extensions: vec![
                    scene::extension(scene_commit_log.clone()),
                    crypto::extension(),
                    timers::extension(),
                    capabilities::extension(capabilities),
                ],
                module_loader: Some(Rc::new(module_loader)),
                ..Default::default()
            }),
            scene_commit_log,
            entrypoint_module: Some(entrypoint_module),
        };
        runtime.bootstrap_unprivileged_globals()?;
        Ok(runtime)
    }

    /// Executes a JavaScript script.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when V8 rejects or throws while evaluating the script.
    pub fn execute_script(
        &mut self,
        name: impl AsRef<str>,
        source: impl AsRef<str>,
    ) -> Result<(), JsRuntimeError> {
        self.runtime
            .execute_script(name.as_ref().to_owned(), source.as_ref().to_owned())
            .map(|_| ())
            .map_err(|error| {
                JsRuntimeError::new(
                    "js-runtime.execute-failed",
                    format!("JavaScript execution failed: {error}"),
                )
            })
    }

    /// Executes the sealed module graph entrypoint.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when no entrypoint is configured, module loading fails,
    /// or module evaluation throws.
    pub fn execute_entrypoint_module(&mut self) -> Result<(), JsRuntimeError> {
        let entrypoint = self.entrypoint_module.clone().ok_or_else(|| {
            JsRuntimeError::new(
                "js-runtime.module.entrypoint-missing",
                "runtime was not created with a sealed module graph entrypoint",
            )
        })?;
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .map_err(|error| {
                JsRuntimeError::new(
                    "js-runtime.module.executor-failed",
                    format!("JavaScript module executor failed to start: {error}"),
                )
            })?;
        let module_id = tokio_runtime
            .block_on(self.runtime.load_main_es_module(&entrypoint))
            .map_err(|error| {
                JsRuntimeError::new(
                    "js-runtime.module.load-failed",
                    format!("JavaScript module graph load failed: {error}"),
                )
            })?;
        let evaluation = {
            let _runtime_guard = tokio_runtime.enter();
            self.runtime.mod_evaluate(module_id)
        };
        tokio_runtime
            .block_on(self.runtime.run_event_loop(PollEventLoopOptions::default()))
            .map_err(|error| {
                JsRuntimeError::new(
                    "js-runtime.module.event-loop-failed",
                    format!("JavaScript module event loop failed: {error}"),
                )
            })?;
        tokio_runtime.block_on(evaluation).map_err(|error| {
            JsRuntimeError::new(
                "js-runtime.module.evaluate-failed",
                format!("JavaScript module evaluation failed: {error}"),
            )
        })
    }

    /// Executes a JavaScript script and converts a primitive result into a host value.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when V8 rejects the script or when the result cannot
    /// be represented as a primitive `Hawk2UI` runtime value.
    pub fn evaluate_script_value(
        &mut self,
        name: impl AsRef<str>,
        source: impl AsRef<str>,
    ) -> Result<JsRuntimeValue, JsRuntimeError> {
        let value =
            self.execute_script_value(name.as_ref().to_owned(), source.as_ref().to_owned())?;
        self.perform_microtask_checkpoint();
        self.primitive_value_from_v8(&value)
    }

    /// Executes a JavaScript script with a wall-clock timeout and converts its primitive result.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when V8 rejects or terminates the script, or when the result
    /// cannot be represented as a primitive `Hawk2UI` runtime value.
    pub fn evaluate_script_value_with_timeout(
        &mut self,
        name: impl AsRef<str>,
        source: impl AsRef<str>,
        timeout: Duration,
    ) -> Result<JsRuntimeValue, JsRuntimeError> {
        let value = self.execute_script_value_with_timeout_inner(
            name.as_ref().to_owned(),
            source.as_ref().to_owned(),
            timeout,
        )?;
        self.perform_microtask_checkpoint();
        self.primitive_value_from_v8(&value)
    }

    /// Returns the validated scene batches committed by JavaScript.
    #[must_use]
    pub fn scene_batches(&self) -> Vec<SceneOpBatch> {
        self.scene_commit_log.snapshot()
    }

    /// Runs queued V8 microtasks for deterministic host-driven script evaluation.
    pub fn perform_microtask_checkpoint(&mut self) {
        let runtime = &mut self.runtime;
        deno_core::scope!(scope, runtime);
        scope.perform_microtask_checkpoint();
    }

    fn execute_script_value(
        &mut self,
        name: String,
        source: String,
    ) -> Result<v8::Global<v8::Value>, JsRuntimeError> {
        self.runtime.execute_script(name, source).map_err(|error| {
            JsRuntimeError::new(
                "js-runtime.execute-failed",
                format!("JavaScript execution failed: {error}"),
            )
        })
    }

    fn execute_script_value_with_timeout_inner(
        &mut self,
        name: String,
        source: String,
        timeout: Duration,
    ) -> Result<v8::Global<v8::Value>, JsRuntimeError> {
        let isolate_handle = self.runtime.v8_isolate().thread_safe_handle();
        let (finished_tx, finished_rx) = mpsc::channel();
        let terminator = thread::spawn(move || {
            if finished_rx.recv_timeout(timeout).is_err() {
                isolate_handle.terminate_execution()
            } else {
                false
            }
        });

        let result = self.execute_script_value(name, source);
        let _ = finished_tx.send(());
        let terminated = terminator.join().unwrap_or(false);
        if terminated {
            let _ = self.runtime.v8_isolate().cancel_terminate_execution();
        }
        result
    }

    fn primitive_value_from_v8(
        &mut self,
        value: &v8::Global<v8::Value>,
    ) -> Result<JsRuntimeValue, JsRuntimeError> {
        let runtime = &mut self.runtime;
        deno_core::scope!(scope, runtime);
        let value = value.open(scope);
        if value.is_null_or_undefined() {
            Ok(JsRuntimeValue::Null)
        } else if value.is_boolean() {
            Ok(JsRuntimeValue::Bool(value.boolean_value(scope)))
        } else if value.is_number() {
            value
                .number_value(scope)
                .map(JsRuntimeValue::Number)
                .ok_or_else(|| {
                    JsRuntimeError::new(
                        "js-runtime.value.unsupported",
                        "JavaScript number result cannot be represented as a structured Hawk2UI value",
                    )
                })
        } else if value.is_string() {
            value
                .to_string(scope)
                .map(|value| JsRuntimeValue::String(value.to_rust_string_lossy(scope)))
                .ok_or_else(|| {
                    JsRuntimeError::new(
                        "js-runtime.value.unsupported-string",
                        "JavaScript string result cannot be represented as UTF-8",
                    )
                })
        } else {
            Err(JsRuntimeError::new(
                "js-runtime.value.unsupported",
                "JavaScript result type cannot be represented as a structured Hawk2UI value",
            ))
        }
    }

    fn bootstrap_unprivileged_globals(&mut self) -> Result<(), JsRuntimeError> {
        self.execute_script(
            "hawk2ui:bootstrap/unprivileged-globals",
            UNPRIVILEGED_GLOBALS_BOOTSTRAP,
        )
    }
}

const UNPRIVILEGED_GLOBALS_BOOTSTRAP: &str = r##"
{
const commitScene = Deno.core.ops.op_hawk_scene_commit;
const cryptoGetRandomValues = Deno.core.ops.op_hawk_crypto_get_random_values;
const cryptoDigest = Deno.core.ops.op_hawk_crypto_digest;
const timerDelay = Deno.core.ops.op_hawk_timer_delay;
const networkRequest = Deno.core.ops.op_hawk_network_request;
const aiCallProvider = Deno.core.ops.op_hawk_ai_call_provider;
const aiStreamProvider = Deno.core.ops.op_hawk_ai_stream_provider;
const apiCall = Deno.core.ops.op_hawk_api_call;
const secretsRead = Deno.core.ops.op_hawk_secrets_read;
const desktopSetWindowTitle = Deno.core.ops.op_hawk_desktop_set_window_title;
const desktopShowOpenDialog = Deno.core.ops.op_hawk_desktop_show_open_dialog;
const desktopReadClipboard = Deno.core.ops.op_hawk_desktop_read_clipboard;
const desktopWriteClipboard = Deno.core.ops.op_hawk_desktop_write_clipboard;
const desktopNotify = Deno.core.ops.op_hawk_desktop_notify;
const desktopRegisterShortcut = Deno.core.ops.op_hawk_desktop_register_shortcut;
const desktopOpenExternal = Deno.core.ops.op_hawk_desktop_open_external;
const desktopNextDeepLink = Deno.core.ops.op_hawk_desktop_next_deep_link;
const desktopSetWindowMode = Deno.core.ops.op_hawk_desktop_set_window_mode;
const desktopCloseWindow = Deno.core.ops.op_hawk_desktop_close_window;
const storageGetItem = Deno.core.ops.op_hawk_storage_get_item;
const storageSetItem = Deno.core.ops.op_hawk_storage_set_item;
const storageGetDocument = Deno.core.ops.op_hawk_storage_get_document;
const storagePutDocument = Deno.core.ops.op_hawk_storage_put_document;
const storageTransaction = Deno.core.ops.op_hawk_storage_transaction;
const filesReadText = Deno.core.ops.op_hawk_files_read_text;
const filesWriteText = Deno.core.ops.op_hawk_files_write_text;
const filesReadBytes = Deno.core.ops.op_hawk_files_read_bytes;
const filesWriteBytes = Deno.core.ops.op_hawk_files_write_bytes;
const filesPick = Deno.core.ops.op_hawk_files_pick;
const filesPickFolder = Deno.core.ops.op_hawk_files_pick_folder;
const filesWatch = Deno.core.ops.op_hawk_files_watch;
const filesImport = Deno.core.ops.op_hawk_files_import;
const filesExport = Deno.core.ops.op_hawk_files_export;
const pluginReadParameter = Deno.core.ops.op_hawk_plugin_read_parameter;
const pluginWriteParameter = Deno.core.ops.op_hawk_plugin_write_parameter;
const pluginBeginAutomationGesture = Deno.core.ops.op_hawk_plugin_begin_automation_gesture;
const pluginEndAutomationGesture = Deno.core.ops.op_hawk_plugin_end_automation_gesture;
const pluginLoadState = Deno.core.ops.op_hawk_plugin_load_state;
const pluginSaveState = Deno.core.ops.op_hawk_plugin_save_state;
const pluginLoadPreset = Deno.core.ops.op_hawk_plugin_load_preset;
const pluginSavePreset = Deno.core.ops.op_hawk_plugin_save_preset;
const pluginGetTransport = Deno.core.ops.op_hawk_plugin_get_transport;
const pluginResizeEditor = Deno.core.ops.op_hawk_plugin_resize_editor;
const pluginFocusEditor = Deno.core.ops.op_hawk_plugin_focus_editor;
const audioSubscribeMeters = Deno.core.ops.op_hawk_audio_subscribe_meters;
  const audioTransport = Deno.core.ops.op_hawk_audio_transport;
  const audioNextControl = Deno.core.ops.op_hawk_audio_next_control;
  const dspSendControl = Deno.core.ops.op_hawk_dsp_send_control;
  const dspUpdateParameterGraph = Deno.core.ops.op_hawk_dsp_update_parameter_graph;
  const dspStartAnalysisJob = Deno.core.ops.op_hawk_dsp_start_analysis_job;
  const dspCancelAnalysisJob = Deno.core.ops.op_hawk_dsp_cancel_analysis_job;
  const dspStartOfflineRender = Deno.core.ops.op_hawk_dsp_start_offline_render;
const dspExportOfflineRender = Deno.core.ops.op_hawk_dsp_export_offline_render;
function unsupportedWebApi(name) {
  throw new Error(
    `js-runtime.web-api.unsupported: ${name} is not available in the Hawk2UI native runtime; use hawk:* capability APIs or native scene operations.`
  );
}
  function unsupportedWebApiProxy(prefix) {
    return new Proxy(Object.freeze({}), {
      get(_target, property) {
        if (property === Symbol.toStringTag) return prefix;
        if (property === "toString") return () => `[unsupported ${prefix}]`;
      unsupportedWebApi(`${prefix}.${String(property)}`);
    },
    set(_target, property) {
      unsupportedWebApi(`${prefix}.${String(property)}`);
    },
    apply() {
      unsupportedWebApi(prefix);
      }
    });
  }
  function unsupportedWebApiConstructor(name) {
    return new Proxy(function unsupportedConstructor() {}, {
      apply() {
        unsupportedWebApi(name);
      },
      construct() {
        unsupportedWebApi(name);
      },
      get(_target, property) {
        if (property === "prototype") return Object.freeze({});
        if (property === "toString") return () => `[unsupported ${name}]`;
        unsupportedWebApi(`${name}.${String(property)}`);
      },
      set(_target, property) {
        unsupportedWebApi(`${name}.${String(property)}`);
      }
    });
  }
  const unsupportedDocument = unsupportedWebApiProxy("document");
  const unsupportedWindow = unsupportedWebApiProxy("window");
  const unsupportedLocalStorage = unsupportedWebApiProxy("localStorage");
  const unsupportedSessionStorage = unsupportedWebApiProxy("sessionStorage");
  const unsupportedNavigator = unsupportedWebApiProxy("navigator");
  const unsupportedLocation = unsupportedWebApiProxy("location");
  const unsupportedWebSocket = unsupportedWebApiConstructor("WebSocket");
  const unsupportedXMLHttpRequest = unsupportedWebApiConstructor("XMLHttpRequest");
  const unsupportedEventSource = unsupportedWebApiConstructor("EventSource");
function normalizePathname(pathname) {
  const hasTrailingSlash = pathname.endsWith("/");
  const parts = [];
  for (const segment of pathname.split("/")) {
    if (segment === "" || segment === ".") continue;
    if (segment === "..") parts.pop();
    else parts.push(segment);
  }
  const normalized = `/${parts.join("/")}`;
  return hasTrailingSlash && normalized !== "/" ? `${normalized}/` : normalized;
}
function hasUrlScheme(value) {
  return /^[A-Za-z][A-Za-z0-9+.-]*:/.test(value);
}
function parseUrlAuthority(authority) {
  const portSeparator = authority.lastIndexOf(":");
  if (portSeparator > -1 && !authority.includes("]")) {
    return {
      host: authority,
      hostname: authority.slice(0, portSeparator),
      port: authority.slice(portSeparator + 1)
    };
  }
  return { host: authority, hostname: authority, port: "" };
}
function parseAbsoluteUrl(value) {
  const match = String(value).match(/^([A-Za-z][A-Za-z0-9+.-]*:)(?:\/\/([^/?#]*))?([^?#]*)(\?[^#]*)?(#.*)?$/);
  if (!match) throw new TypeError(`Invalid URL: ${value}`);
  const authority = match[2] ?? "";
  const authorityParts = parseUrlAuthority(authority);
  const pathname = authority
    ? normalizePathname(match[3] || "/")
    : normalizePathname(match[3] || "");
  return {
    protocol: match[1].toLowerCase(),
    host: authorityParts.host,
    hostname: authorityParts.hostname,
    port: authorityParts.port,
    pathname,
    search: match[4] ?? "",
    hash: match[5] ?? ""
  };
}
function resolveUrlInput(input, base = undefined) {
  const value = String(input);
  if (hasUrlScheme(value)) return value;
  if (base === undefined) throw new TypeError(`Invalid URL: ${value}`);
  const baseUrl = base instanceof URL ? base : new URL(base);
  if (value.startsWith("//")) return `${baseUrl.protocol}${value}`;
  if (value.startsWith("/")) return `${baseUrl.origin}${value}`;
  if (value.startsWith("?")) return `${baseUrl.origin}${baseUrl.pathname}${value}`;
  if (value.startsWith("#")) return `${baseUrl.origin}${baseUrl.pathname}${baseUrl.search}${value}`;
  const baseDirectory = baseUrl.pathname.endsWith("/")
    ? baseUrl.pathname
    : baseUrl.pathname.slice(0, baseUrl.pathname.lastIndexOf("/") + 1);
  return `${baseUrl.origin}${normalizePathname(`${baseDirectory}${value}`)}`;
}
function decodeQueryComponent(value) {
  return decodeURIComponent(String(value).replace(/\+/g, " "));
}
function encodeQueryComponent(value) {
  return encodeURIComponent(String(value));
}
class URLSearchParams {
  constructor(init = "") {
    this.__pairs = [];
    this.__onchange = undefined;
    if (init instanceof URLSearchParams) {
      for (const [name, value] of init) this.__pairs.push([name, value]);
      return;
    }
    if (typeof init === "string") {
      const query = init.startsWith("?") ? init.slice(1) : init;
      if (query.length === 0) return;
      for (const part of query.split("&")) {
        if (part.length === 0) continue;
        const separator = part.indexOf("=");
        const name = separator === -1 ? part : part.slice(0, separator);
        const value = separator === -1 ? "" : part.slice(separator + 1);
        this.__pairs.push([decodeQueryComponent(name), decodeQueryComponent(value)]);
      }
      return;
    }
    if (Array.isArray(init)) {
      for (const pair of init) {
        if (!Array.isArray(pair) || pair.length !== 2) {
          throw new TypeError("URLSearchParams init sequence entries must be [name, value] pairs.");
        }
        this.__pairs.push([String(pair[0]), String(pair[1])]);
      }
      return;
    }
    if (init && typeof init === "object") {
      for (const [name, value] of Object.entries(init)) this.__pairs.push([name, String(value)]);
      return;
    }
  }
  __setChangeCallback(callback) {
    this.__onchange = callback;
  }
  __notifyChange() {
    if (this.__onchange) this.__onchange(this.toString());
  }
  append(name, value) {
    this.__pairs.push([String(name), String(value)]);
    this.__notifyChange();
  }
  delete(name) {
    const key = String(name);
    this.__pairs = this.__pairs.filter(([current]) => current !== key);
    this.__notifyChange();
  }
  get(name) {
    const key = String(name);
    const pair = this.__pairs.find(([current]) => current === key);
    return pair ? pair[1] : null;
  }
  getAll(name) {
    const key = String(name);
    return this.__pairs.filter(([current]) => current === key).map(([, value]) => value);
  }
  has(name) {
    const key = String(name);
    return this.__pairs.some(([current]) => current === key);
  }
  set(name, value) {
    const key = String(name);
    const nextValue = String(value);
    let replaced = false;
    const nextPairs = [];
    for (const pair of this.__pairs) {
      if (pair[0] !== key) {
        nextPairs.push(pair);
      } else if (!replaced) {
        nextPairs.push([key, nextValue]);
        replaced = true;
      }
    }
    if (!replaced) nextPairs.push([key, nextValue]);
    this.__pairs = nextPairs;
    this.__notifyChange();
  }
  sort() {
    this.__pairs.sort(([left], [right]) => left.localeCompare(right));
    this.__notifyChange();
  }
  entries() {
    return this.__pairs[Symbol.iterator]();
  }
  keys() {
    return this.__pairs.map(([name]) => name)[Symbol.iterator]();
  }
  values() {
    return this.__pairs.map(([, value]) => value)[Symbol.iterator]();
  }
  forEach(callback, thisArg = undefined) {
    for (const [name, value] of this.__pairs) callback.call(thisArg, value, name, this);
  }
  [Symbol.iterator]() {
    return this.entries();
  }
  toString() {
    return this.__pairs
      .map(([name, value]) => `${encodeQueryComponent(name)}=${encodeQueryComponent(value)}`)
      .join("&");
  }
}
class URL {
  constructor(input, base = undefined) {
    this.__applyParsed(parseAbsoluteUrl(resolveUrlInput(input, base)));
  }
  __applyParsed(parsed) {
    this.protocol = parsed.protocol;
    this.host = parsed.host;
    this.hostname = parsed.hostname;
    this.port = parsed.port;
    this.pathname = parsed.pathname || "/";
    this.__search = parsed.search;
    this.hash = parsed.hash;
    this.searchParams = new URLSearchParams(this.__search);
    this.searchParams.__setChangeCallback((query) => {
      this.__search = query ? `?${query}` : "";
    });
  }
  get href() {
    const authority = this.host ? `//${this.host}` : "";
    return `${this.protocol}${authority}${this.pathname}${this.__search}${this.hash}`;
  }
  set href(value) {
    this.__applyParsed(parseAbsoluteUrl(resolveUrlInput(value)));
  }
  get origin() {
    return this.host ? `${this.protocol}//${this.host}` : "null";
  }
  get search() {
    return this.__search;
  }
  set search(value) {
    const next = String(value);
    this.__search = next.length === 0 ? "" : next.startsWith("?") ? next : `?${next}`;
    this.searchParams = new URLSearchParams(this.__search);
    this.searchParams.__setChangeCallback((query) => {
      this.__search = query ? `?${query}` : "";
    });
  }
  toJSON() {
    return this.href;
  }
  toString() {
    return this.href;
  }
}
function utf8Encode(value) {
  const bytes = [];
  const input = String(value);
  for (let index = 0; index < input.length; index += 1) {
    let codePoint = input.charCodeAt(index);
    if (codePoint >= 0xd800 && codePoint <= 0xdbff && index + 1 < input.length) {
      const next = input.charCodeAt(index + 1);
      if (next >= 0xdc00 && next <= 0xdfff) {
        codePoint = 0x10000 + ((codePoint - 0xd800) << 10) + (next - 0xdc00);
        index += 1;
      }
    }
    if (codePoint <= 0x7f) {
      bytes.push(codePoint);
    } else if (codePoint <= 0x7ff) {
      bytes.push(0xc0 | (codePoint >> 6), 0x80 | (codePoint & 0x3f));
    } else if (codePoint <= 0xffff) {
      bytes.push(0xe0 | (codePoint >> 12), 0x80 | ((codePoint >> 6) & 0x3f), 0x80 | (codePoint & 0x3f));
    } else {
      bytes.push(
        0xf0 | (codePoint >> 18),
        0x80 | ((codePoint >> 12) & 0x3f),
        0x80 | ((codePoint >> 6) & 0x3f),
        0x80 | (codePoint & 0x3f)
      );
    }
  }
  return new Uint8Array(bytes);
}
function utf8Decode(input) {
  const bytes = input instanceof ArrayBuffer
    ? new Uint8Array(input)
    : ArrayBuffer.isView(input)
      ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
      : new Uint8Array(input);
  let output = "";
  for (let index = 0; index < bytes.length;) {
    const first = bytes[index++];
    let codePoint = first;
    if (first >= 0xc2 && first <= 0xdf && index < bytes.length) {
      codePoint = ((first & 0x1f) << 6) | (bytes[index++] & 0x3f);
    } else if (first >= 0xe0 && first <= 0xef && index + 1 < bytes.length) {
      codePoint = ((first & 0x0f) << 12) | ((bytes[index++] & 0x3f) << 6) | (bytes[index++] & 0x3f);
    } else if (first >= 0xf0 && first <= 0xf4 && index + 2 < bytes.length) {
      codePoint = ((first & 0x07) << 18)
        | ((bytes[index++] & 0x3f) << 12)
        | ((bytes[index++] & 0x3f) << 6)
        | (bytes[index++] & 0x3f);
    }
    output += String.fromCodePoint(codePoint);
  }
  return output;
}
class TextEncoder {
  constructor() {
    this.encoding = "utf-8";
  }
  encode(input = "") {
    return utf8Encode(input);
  }
  encodeInto(input, destination) {
    const encoded = utf8Encode(input);
    const written = Math.min(encoded.length, destination.length);
    destination.set(encoded.subarray(0, written));
    return { read: String(input).length, written };
  }
}
class TextDecoder {
  constructor(label = "utf-8", options = {}) {
    const normalized = String(label).trim().toLowerCase();
    if (!["utf-8", "utf8", "unicode-1-1-utf-8"].includes(normalized)) {
      throw new RangeError("TextDecoder only supports utf-8 in the Hawk2UI runtime.");
    }
    this.encoding = "utf-8";
    this.fatal = Boolean(options?.fatal);
    this.ignoreBOM = Boolean(options?.ignoreBOM);
  }
  decode(input = new Uint8Array(), _options = {}) {
    return utf8Decode(input);
  }
}
  let nextTimerId = 1;
  const activeTimeouts = new Set();
  function timeoutDelayMs(delay) {
    const number = Number(delay);
    if (!Number.isFinite(number) || number <= 0) return 0;
    return Math.min(Math.trunc(number), 2147483647);
  }
  function scheduledDelay(delayMs) {
    return delayMs > 0 ? timerDelay(delayMs) : Promise.resolve();
  }
  const hawkSetTimeout = (callback, delay = 0, ...args) => {
    if (typeof callback !== "function") {
      throw new TypeError("setTimeout callback must be a function in the Hawk2UI runtime.");
    }
    const id = nextTimerId++;
    activeTimeouts.add(id);
    const delayMs = timeoutDelayMs(delay);
    scheduledDelay(delayMs).then(() => {
      if (!activeTimeouts.has(id)) return;
      activeTimeouts.delete(id);
      callback(...args);
    });
    return id;
  };
  const hawkClearTimeout = (id) => {
    activeTimeouts.delete(Number(id));
  };
  async function runInterval(id, delayMs, callback, args) {
    while (activeTimeouts.has(id)) {
      await scheduledDelay(delayMs);
      if (!activeTimeouts.has(id)) return;
      callback(...args);
    }
  }
  const hawkSetInterval = (callback, delay = 0, ...args) => {
    if (typeof callback !== "function") {
      throw new TypeError("setInterval callback must be a function in the Hawk2UI runtime.");
    }
    const id = nextTimerId++;
    activeTimeouts.add(id);
    const delayMs = Math.max(1, timeoutDelayMs(delay));
    runInterval(id, delayMs, callback, args);
    return id;
  };
  const hawkClearInterval = hawkClearTimeout;
  function bytesFromBufferSource(data) {
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  throw new TypeError("Expected an ArrayBuffer or typed array.");
}
function isIntegerTypedArray(value) {
  return ArrayBuffer.isView(value)
    && !(value instanceof DataView)
    && !(value instanceof Float32Array)
    && !(value instanceof Float64Array);
}
function cryptoRandomUUID() {
  const bytes = cryptoGetRandomValues(16);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = bytes.map((byte) => byte.toString(16).padStart(2, "0"));
  return [
    hex.slice(0, 4).join(""),
    hex.slice(4, 6).join(""),
    hex.slice(6, 8).join(""),
    hex.slice(8, 10).join(""),
    hex.slice(10, 16).join("")
  ].join("-");
}
const cryptoSubtle = Object.freeze({
  async digest(algorithm, data) {
    const name = typeof algorithm === "string" ? algorithm : algorithm?.name;
    if (!name) throw new TypeError("crypto.subtle.digest requires an algorithm name.");
    const bytes = Array.from(bytesFromBufferSource(data));
    const digest = cryptoDigest({ algorithm: String(name), bytes });
    return new Uint8Array(digest).buffer;
  }
});
const hawkCrypto = Object.freeze({
  getRandomValues(array) {
    if (!isIntegerTypedArray(array)) {
      throw new TypeError("crypto.getRandomValues requires an integer typed array.");
    }
    const bytes = cryptoGetRandomValues(array.byteLength);
    new Uint8Array(array.buffer, array.byteOffset, array.byteLength).set(bytes);
    return array;
  },
  randomUUID: cryptoRandomUUID,
  subtle: cryptoSubtle
});
function normalizeHeaderName(name) {
  return String(name).trim().toLowerCase();
}
function normalizeHeaderValue(value) {
  return String(value);
}
class Headers {
  #values = new Map();
  constructor(init = undefined) {
    if (init === undefined || init === null) return;
    if (init instanceof Headers) {
      for (const [name, value] of init) this.append(name, value);
      return;
    }
    if (Array.isArray(init)) {
      for (const pair of init) {
        if (!Array.isArray(pair) || pair.length !== 2) {
          throw new TypeError("Headers init sequence entries must be [name, value] pairs.");
        }
        this.append(pair[0], pair[1]);
      }
      return;
    }
    if (typeof init === "object") {
      for (const [name, value] of Object.entries(init)) this.append(name, value);
      return;
    }
    throw new TypeError("Headers init must be a Headers object, sequence, or record.");
  }
  append(name, value) {
    const key = normalizeHeaderName(name);
    const normalized = normalizeHeaderValue(value);
    const previous = this.#values.get(key);
    this.#values.set(key, previous === undefined ? normalized : `${previous}, ${normalized}`);
  }
  set(name, value) {
    this.#values.set(normalizeHeaderName(name), normalizeHeaderValue(value));
  }
  get(name) {
    return this.#values.get(normalizeHeaderName(name)) ?? null;
  }
  has(name) {
    return this.#values.has(normalizeHeaderName(name));
  }
  delete(name) {
    this.#values.delete(normalizeHeaderName(name));
  }
  entries() {
    return this.#values.entries();
  }
  keys() {
    return this.#values.keys();
  }
  values() {
    return this.#values.values();
  }
  forEach(callback, thisArg = undefined) {
    for (const [name, value] of this.#values) callback.call(thisArg, value, name, this);
  }
  [Symbol.iterator]() {
    return this.entries();
  }
  toJSON() {
    return Object.fromEntries(this.#values);
  }
}
class AbortSignal {
  constructor() {
    this.aborted = false;
    this.reason = undefined;
    this.__listeners = [];
  }
  addEventListener(type, listener) {
    if (type === "abort" && typeof listener === "function") this.__listeners.push(listener);
  }
  removeEventListener(type, listener) {
    if (type !== "abort") return;
    this.__listeners = this.__listeners.filter((current) => current !== listener);
  }
  dispatchEvent(event) {
    if (event?.type !== "abort") return true;
    for (const listener of [...this.__listeners]) listener.call(this, event);
    return true;
  }
}
class AbortController {
  constructor() {
    this.signal = new AbortSignal();
  }
  abort(reason = "aborted") {
    if (this.signal.aborted) return;
    this.signal.aborted = true;
    this.signal.reason = reason;
    this.signal.dispatchEvent({ type: "abort", target: this.signal });
  }
}
  class Request {
    constructor(input, init = {}) {
      const source = input instanceof Request ? input : undefined;
      this.url = source ? source.url : String(input);
      this.method = String(init.method ?? source?.method ?? "GET").toUpperCase();
      this.headers = new Headers(init.headers ?? source?.headers);
      this.body = init.body ?? source?.body ?? null;
      this.signal = init.signal ?? source?.signal ?? null;
      this.timeoutMs = init.timeoutMs ?? source?.timeoutMs;
      this.redirect = String(init.redirect ?? source?.redirect ?? "follow").toLowerCase();
    }
    clone() {
      return new Request(this);
    }
  }
class Response {
  constructor(body = "", init = {}) {
    this.status = Number(init.status ?? 200);
    this.headers = new Headers(init.headers);
    this.body = body == null ? "" : String(body);
    this.ok = this.status >= 200 && this.status <= 299;
  }
  async text() {
    return this.body;
  }
  async json() {
    return JSON.parse(this.body);
  }
  clone() {
      return new Response(this.body, { status: this.status, headers: this.headers });
    }
  }
  const FETCH_REDIRECT_POLICIES = new Set(["follow", "error", "manual"]);
  function fetchAbortAfterDispatchPromise(signal) {
    if (!signal || typeof signal.addEventListener !== "function") {
      return { promise: null, cleanup: () => {} };
    }
    let abortListener;
    const promise = new Promise((_, reject) => {
      abortListener = () => {
        reject(new Error("js-runtime.capability.aborted: fetch request was aborted after dispatch"));
      };
      signal.addEventListener("abort", abortListener);
    });
    return {
      promise,
      cleanup: () => signal.removeEventListener?.("abort", abortListener)
    };
  }
  function responseBlockedByRedirectPolicy(request, response) {
    if (request.redirect !== "error") return null;
    if (response.status < 300 || response.status > 399) return null;
    const location = response.headers.get("location");
    if (location === null) return null;
    return new Error(`js-runtime.capability.denied: fetch redirect policy rejected redirect response to ${location}`);
  }
  async function fetch(input, init = {}) {
    const request = input instanceof Request ? new Request(input, init) : new Request(input, init);
    if (request.signal?.aborted) {
      throw new Error("js-runtime.capability.aborted: fetch request was aborted before dispatch");
    }
    if (request.timeoutMs !== undefined) {
      const timeoutMs = Number(request.timeoutMs);
      if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
        throw new Error("js-runtime.capability.invalid: fetch timeoutMs must be greater than zero");
      }
      request.timeoutMs = timeoutMs;
    }
    if (!FETCH_REDIRECT_POLICIES.has(request.redirect)) {
      throw new Error("js-runtime.capability.invalid: fetch redirect must be follow, error, or manual");
    }
    const responsePromise = Promise.resolve(networkRequest(request.url, {
      method: request.method,
      headers: request.headers.toJSON(),
      body: request.body,
      timeoutMs: request.timeoutMs
    })).then((response) => {
      const fetchResponse = new Response(response.body, { status: response.status, headers: response.headers });
      const redirectError = responseBlockedByRedirectPolicy(request, fetchResponse);
      if (redirectError) throw redirectError;
      return fetchResponse;
    });
    const abort = fetchAbortAfterDispatchPromise(request.signal);
    if (!abort.promise) return await responsePromise;
    try {
      return await Promise.race([responsePromise, abort.promise]);
    } finally {
      abort.cleanup();
    }
  }
Object.defineProperty(globalThis, "__hawk2uiCommitScene", {
  value(batch) {
    return commitScene(batch);
  },
  writable: false,
  enumerable: false,
  configurable: false
});
Object.defineProperties(globalThis, {
  __hawk2uiNetworkRequest: {
    value(url, init) {
      return networkRequest(url, init);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiAiCallProvider: {
    value(provider, payload, options) {
      return aiCallProvider(provider, payload, options);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiAiStreamProvider: {
    value(provider, payload, options) {
      return aiStreamProvider(provider, payload, options);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiApiCall: {
    value(name, payload, options) {
      return apiCall(name, payload, options);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiSecretsRead: {
    value(name) {
      return secretsRead(name);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopSetWindowTitle: {
    value(title) {
      return desktopSetWindowTitle(title);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopShowOpenDialog: {
    value(options) {
      return desktopShowOpenDialog(options);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopReadClipboard: {
    value() {
      return desktopReadClipboard();
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopWriteClipboard: {
    value(text) {
      return desktopWriteClipboard(text);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopNotify: {
    value(notification) {
      return desktopNotify(notification);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopRegisterShortcut: {
    value(shortcut) {
      return desktopRegisterShortcut(shortcut);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopOpenExternal: {
    value(url) {
      return desktopOpenExternal(url);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopNextDeepLink: {
    value(scheme) {
      return desktopNextDeepLink(scheme);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopSetWindowMode: {
    value(mode) {
      return desktopSetWindowMode(mode);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDesktopCloseWindow: {
    value(reason) {
      return desktopCloseWindow(reason);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiStorageGetItem: {
    value(namespace, key) {
      return storageGetItem(namespace, key);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
    __hawk2uiStorageSetItem: {
      value(namespace, key, value) {
        return storageSetItem(namespace, key, value);
      },
      writable: false,
      enumerable: false,
      configurable: false
    },
    __hawk2uiStorageGetDocument: {
      value(namespace, key) {
        return storageGetDocument(namespace, key);
      },
      writable: false,
      enumerable: false,
      configurable: false
    },
    __hawk2uiStoragePutDocument: {
      value(namespace, key, value) {
        return storagePutDocument(namespace, key, value);
      },
      writable: false,
      enumerable: false,
      configurable: false
    },
    __hawk2uiStorageTransaction: {
      value(namespace, writes) {
        return storageTransaction(namespace, writes);
      },
      writable: false,
      enumerable: false,
      configurable: false
    },
    __hawk2uiFilesReadText: {
    value(path) {
      return filesReadText(path);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiFilesWriteText: {
    value(path, text) {
      return filesWriteText(path, text);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiFilesReadBytes: {
    value(path) {
      return filesReadBytes(path);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiFilesWriteBytes: {
    value(path, bytes) {
      return filesWriteBytes(path, bytes);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiFilesPick: {
    value() {
      return filesPick();
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiFilesPickFolder: {
    value() {
      return filesPickFolder();
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiFilesWatch: {
    value(path) {
      return filesWatch(path);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiFilesImport: {
    value(destinationPath) {
      return filesImport(destinationPath);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiFilesExport: {
    value(sourcePath) {
      return filesExport(sourcePath);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginReadParameter: {
    value(parameter) {
      return pluginReadParameter(parameter);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginWriteParameter: {
    value(parameter, value) {
      return pluginWriteParameter(parameter, value);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginBeginAutomationGesture: {
    value(parameter) {
      return pluginBeginAutomationGesture(parameter);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginEndAutomationGesture: {
    value(parameter) {
      return pluginEndAutomationGesture(parameter);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginLoadState: {
    value() {
      return pluginLoadState();
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginSaveState: {
    value(stateBlob) {
      return pluginSaveState(stateBlob);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginLoadPreset: {
    value(presetId) {
      return pluginLoadPreset(presetId);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginSavePreset: {
    value(presetId, stateBlob) {
      return pluginSavePreset(presetId, stateBlob);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginGetTransport: {
    value() {
      return pluginGetTransport();
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginResizeEditor: {
    value(width, height) {
      return pluginResizeEditor(width, height);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiPluginFocusEditor: {
    value() {
      return pluginFocusEditor();
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiAudioSubscribeMeters: {
    value(options) {
      return audioSubscribeMeters(options);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiAudioTransport: {
    value() {
      return audioTransport();
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiAudioNextControl: {
    value(options) {
      return audioNextControl(options);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDspSendControl: {
    value(message) {
      return dspSendControl(message);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDspUpdateParameterGraph: {
    value(graph) {
      return dspUpdateParameterGraph(graph);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDspStartAnalysisJob: {
    value(request) {
      return dspStartAnalysisJob(request);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDspCancelAnalysisJob: {
    value(id) {
      return dspCancelAnalysisJob(id);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDspStartOfflineRender: {
    value(request) {
      return dspStartOfflineRender(request);
    },
    writable: false,
    enumerable: false,
    configurable: false
  },
  __hawk2uiDspExportOfflineRender: {
    value(id) {
      return dspExportOfflineRender(id);
    },
    writable: false,
    enumerable: false,
    configurable: false
  }
});
Object.defineProperties(globalThis, {
  Headers: {
    value: Headers,
    writable: false,
    enumerable: false,
    configurable: false
  },
  Request: {
    value: Request,
    writable: false,
    enumerable: false,
    configurable: false
  },
  Response: {
    value: Response,
    writable: false,
    enumerable: false,
    configurable: false
  },
  AbortSignal: {
    value: AbortSignal,
    writable: false,
    enumerable: false,
    configurable: false
  },
  AbortController: {
    value: AbortController,
    writable: false,
    enumerable: false,
    configurable: false
  },
  URL: {
    value: URL,
    writable: false,
    enumerable: false,
    configurable: false
  },
  URLSearchParams: {
    value: URLSearchParams,
    writable: false,
    enumerable: false,
    configurable: false
  },
  TextEncoder: {
    value: TextEncoder,
    writable: false,
    enumerable: false,
    configurable: false
  },
  TextDecoder: {
    value: TextDecoder,
    writable: false,
    enumerable: false,
    configurable: false
  },
  setTimeout: {
    value: hawkSetTimeout,
    writable: false,
    enumerable: false,
    configurable: false
  },
  clearTimeout: {
    value: hawkClearTimeout,
    writable: false,
    enumerable: false,
    configurable: false
  },
  setInterval: {
    value: hawkSetInterval,
    writable: false,
    enumerable: false,
    configurable: false
  },
  clearInterval: {
    value: hawkClearInterval,
    writable: false,
    enumerable: false,
    configurable: false
  },
  crypto: {
    value: hawkCrypto,
    writable: false,
    enumerable: false,
    configurable: false
  },
  document: {
    value: unsupportedDocument,
    writable: false,
    enumerable: false,
    configurable: false
  },
  window: {
    value: unsupportedWindow,
    writable: false,
    enumerable: false,
    configurable: false
  },
  localStorage: {
    value: unsupportedLocalStorage,
    writable: false,
    enumerable: false,
    configurable: false
  },
  sessionStorage: {
    value: unsupportedSessionStorage,
    writable: false,
    enumerable: false,
    configurable: false
  },
  navigator: {
    value: unsupportedNavigator,
    writable: false,
    enumerable: false,
    configurable: false
  },
    location: {
      value: unsupportedLocation,
      writable: false,
      enumerable: false,
      configurable: false
    },
    WebSocket: {
      value: unsupportedWebSocket,
      writable: false,
      enumerable: false,
      configurable: false
    },
    XMLHttpRequest: {
      value: unsupportedXMLHttpRequest,
      writable: false,
      enumerable: false,
      configurable: false
    },
    EventSource: {
      value: unsupportedEventSource,
      writable: false,
      enumerable: false,
      configurable: false
    },
    fetch: {
      value: fetch,
      writable: false,
    enumerable: false,
    configurable: false
  }
});
}
delete globalThis.Deno;
"##;
