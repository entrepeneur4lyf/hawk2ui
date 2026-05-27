# Hawk2UI Runtime APIs

The runtime API surface is the public contract between application code, host adapters, diagnostics, surfaces, scheduling, and host capabilities.

## API Modules

- `Diagnostic`: diagnostics, source spans, suggested fixes, and related context.
- `Runtime`: lifecycle phases, jobs, scheduler-visible work, and host bindings.
- `Surface`: desktop and embedded host surface metrics, input events, and repaint contracts.
- `Artifact`: schema and artifact compatibility records used when loading sealed artifacts.
- `Plugin`: plugin editor, parameter, state, preset, and realtime data contracts.

## Diagnostic Records

- `Diagnostic`: structured diagnostic message.
- `DiagnosticSeverity`: `Info`, `Warning`, or `Error` severity.
- `RuleId`: stable diagnostic rule identifier.
- `SourceSpan`: source location attached to a diagnostic.
- `SuggestedFix`: machine-readable fix hint.
- `RelatedContext`: extra context attached to a diagnostic.

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

## Custom Renderer Protocol

Framework integrations should emit native records through `CustomRendererProtocol` and `CustomRendererOperation`. The protocol validates node identity and records deterministic operation keys before native runtime bridging.

The implemented operation surface covers create node, set prop, set style ref, set asset ref, set native ref, bind event, bind lifecycle, append keyed or unkeyed children, enter error boundary, commit, and remove node. Protocol diagnostics use stable rules such as `custom-renderer.node.duplicate` and `custom-renderer.node.missing`.

React 19+ emits its custom renderer operation list through this protocol. Remaining framework adapters must use the same protocol as they are moved from source scanner compatibility paths to explicit compiler/protocol inputs.

## Framework Native Compiler Boundary

Framework compilers and runtime adapters should hand Rust an explicit `FrameworkNativeProgram` made of `FrameworkNativeNode` records. The program carries the root node, keyed children, props, refs, style refs, asset refs, events, lifecycle handlers, and child node props without requiring Rust to inspect framework source syntax.

Svelte 5, React 19+, Vue 3.5+, and Solid adapters all accept this boundary through `from_native_program(...)`, and framework conformance uses it for normalized snapshot and runtime evidence. Source-string parsing remains a compatibility fixture path; production integrations should emit the typed boundary.
