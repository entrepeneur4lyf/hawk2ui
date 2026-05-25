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
