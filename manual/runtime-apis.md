# Hawk2UI Runtime APIs

The runtime API surface is the public contract between application code, host adapters, diagnostics, surfaces, scheduling, and host capabilities.

## API Modules

- `Diagnostic`: diagnostics, source spans, suggested fixes, and related context.
- `Runtime`: lifecycle phases, jobs, scheduler-visible work, and host bindings.
- `Surface`: desktop and embedded host surface metrics, input events, and repaint contracts.
- `Artifact`: schema and artifact compatibility records used when loading sealed artifacts.
- `Plugin`: plugin editor, parameter, state, preset, and realtime data contracts.
- `Inventory`: public API inventory, module, audience, entry, and stability classification contracts.

## Diagnostic Records

- `Diagnostic`: structured diagnostic message.
- `DiagnosticSeverity`: `Info`, `Warning`, or `Error` severity.
- `RuleId`: stable diagnostic rule identifier.
- `SourceSpan`: source location attached to a diagnostic.
- `SuggestedFix`: machine-readable fix hint.
- `RelatedContext`: extra context attached to a diagnostic.

## Inventory Records

- `ApiInventory`: deterministic public API inventory.
- `ApiModule`: public root API module classification.
- `ApiTypeAudience`: intended consumer group for an API entry.
- `ApiTypeEntry`: single public API inventory entry.
- `ApiTypeStatus`: stability status for an inventory entry.

## Runtime Records

- `CapabilityKey`: declared capability key.
- `RuntimePhase`: lifecycle phase.
- `BindingDirection`: host binding direction.
- `HostBindingContract`: host binding metadata.
- `RuntimeJobId`: stable job identifier.
- `RuntimeJobKind`: kind of runtime job.
- `RuntimeJobStatus`: current job status.
- `RuntimeJob`: scheduled runtime work record.
- `RuntimeLifecycleHook`: lifecycle callback record.

## Surface Records

- `HostSurfaceContract`: host surface kind, metrics, and focus state.
- `SurfaceKind`: desktop, embedded, or offscreen surface kind.
- `SurfaceMetrics`: width, height, and scale metrics.
- `MouseButton`: mouse button enum.
- `KeyModifiers`: key modifier state.
- `KeyEvent`: keyboard event record.
- `InputEvent`: mouse, keyboard, focus, or resize event.
- `RepaintReason`: reason a repaint was requested.
- `RepaintRequest`: repaint request record.
- `FrameSchedule`: frame timing and scheduling record.

## Runtime Use

Applications and framework adapters should communicate with the host through these records instead of relying on backend-specific window or plugin handles. Host capabilities should be declared through manifest capability keys and represented with `CapabilityKey` at runtime.

Runtime event handlers that change visible output should update `RuntimeViewTree` through typed tree operations. `RuntimeViewTree::update_visual(...)` replaces a node visual and invalidates that node so the next `RuntimeSceneBridge` frame carries repaint evidence and updated draw commands.

## Hawk JS Capability Imports

React and Vue sealed-runtime applications and plugin UIs import host capability APIs from stable `hawk:*` modules:

- `hawk:runtime`: aggregate module for the declared runtime capability surface.
- `hawk:network`: bounded HTTP requests for declared hosts.
- `hawk:api`: named declared endpoint calls.
- `hawk:storage`: declared persistent storage keyspaces and migrations.
- `hawk:secrets`: redacted secret handles for declared secrets.
- `hawk:files`: picker-granted file read/write/watch and import/export operations.
- `hawk:desktop`: window commands, dialogs, clipboard, notifications, shortcuts, external URLs, and deep links.
- `hawk:plugin`: plugin parameters, automation gestures, preset/state, host lifecycle, editor resize, and focus.
- `hawk:audio`: transport, tempo, meter, playhead, MIDI/control input, and realtime visual streams.
- `hawk:dsp`: UI-safe DSP control messages, parameter graph updates, analysis, offline render, and export jobs.
- `hawk:ai`: declared provider calls with timeout, budget, streaming, and redaction controls.

The Hawk JS API is default-deny. Capability denials include the manifest path and the required operation so developers can fix `hawk.json` without inspecting host internals. No raw filesystem, network, shell, environment, or secret access is exposed to JavaScript.

Only the documented `hawk:*` modules are injected into the sealed runtime. Unsupported `hawk:*` imports fail with `js-runtime.module.unsupported-hawk-import` so misspelled or undeclared host surfaces are diagnosed before application code can rely on ambient access.

`hawk:network` exposes these network operations: `request`.

`hawk:api` exposes these declared endpoint operations: `call`.

`hawk:storage` exposes these persistent storage and scoped JSON document/database operations: `getItem`, `setItem`, `getDocument`, `putDocument`, `transaction`, and `migrate`.

`hawk:secrets` exposes these secret handle operations: `read`, `isSecretHandle`, and `serializeSecretOptions`.

`hawk:files` exposes these picker-granted file operations: `readText`, `writeText`, `readBytes`, `writeBytes`, `pickFile`, `pickFolder`, `watch`, `importFile`, and `exportFile`.

`hawk:desktop` exposes these desktop host operations: `setWindowTitle`, `showOpenDialog`, `readClipboard`, `writeClipboard`, `notify`, `registerShortcut`, `openExternal`, `onDeepLink`, `setWindowMode`, and `closeWindow`.

`hawk:plugin` exposes these plugin editor and host operations: `readParameter`, `writeParameter`, `beginAutomationGesture`, `endAutomationGesture`, `loadState`, `saveState`, `loadPreset`, `savePreset`, `getTransport`, `resizeEditor`, and `focusEditor`.

`hawk:audio` exposes these UI-safe audio host operations: `subscribeMeters`, `transport`, and `nextControl`.

`hawk:dsp` exposes these UI-safe DSP operations: `sendControl`, `updateParameterGraph`, `startAnalysisJob`, `cancelAnalysisJob`, `startOfflineRender`, and `exportOfflineRender`.

`hawk:ai` exposes these declared provider operations: `callProvider` and `streamProvider`.

`hawk:runtime` re-exports `network`, `api`, `storage`, `secrets`, `files`, `desktop`, `plugin`, `audio`, `dsp`, and `ai`.

## Custom Renderer Protocol

Framework integrations should emit native records through `CustomRendererProtocol` and `CustomRendererOperation`. The protocol validates node identity and records deterministic operation keys before native runtime bridging.

The implemented operation surface covers create node, set prop, set style ref, set asset ref, set native ref, bind event, bind lifecycle, append keyed or unkeyed children, enter error boundary, commit, and remove node. Protocol diagnostics use stable rules such as `custom-renderer.node.duplicate` and `custom-renderer.node.missing`.

## React And Vue Deno Runtime Renderers

Vue 3.5+ and React 19+ are production-supported sealed runtime renderers.

React 19+ production support uses `@hawk2ui/react` `createRoot` with the sealed Deno runtime.

Vue 3.5+ production support uses `@hawk2ui/vue` `createApp` with the sealed Deno runtime.

The `@hawk2ui/react`, `@hawk2ui/vue`, and `@hawk2ui/native` package artifacts are generated from the Hawk2UI repository. Runtime applications consume those npm packages through normal package-manager installs, then Hawk2UI seals the package-manager-produced JavaScript output named by `hawk.json` `build.output`.

React and Vue emit Hawk2UI scene operations through `hawk2ui-js-runtime`, not `FrameworkNativeProgram` or the legacy source-string compiler path.

The React and Vue renderer paths are release-gated by runtime scene operation tests, framework renderer tests, source-mapped diagnostics, package-manager bundle execution, desktop async network/storage/file evidence, and plugin parameter/state/preset/transport/meter/DSP evidence.

React and Vue release artifacts carry sealed JS module graphs, not legacy framework compiler payloads. The graph entrypoint comes from `hawk.json` `build.output`, and package-manager lockfile metadata is recorded with the graph before runtime execution.

Vue production manifests declare `app.framework` as `vue` and package-manager `build.output` as the sealed graph entrypoint.

Vue release artifacts carry sealed JS module graphs, not legacy framework compiler payloads.

## Incubating Framework Compiler Boundary

Framework compilers and runtime adapters should hand Rust an explicit `FrameworkNativeProgram` made of `FrameworkNativeNode` records. The program carries the root node, keyed children, props, refs, style refs, asset refs, events, lifecycle handlers, and child node props without requiring Rust to inspect framework source syntax.

Solid and Svelte remain incubating compiler adapters.

Svelte 5 and Solid adapters can still accept this boundary through `from_native_program(...)`, and framework conformance uses it for normalized snapshot and runtime evidence. Source-string parsing remains a compatibility fixture path; non-React and non-Vue production integrations must move to explicit runtime renderer or compiler/protocol inputs before release claims.
