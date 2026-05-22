# Spec 0004: Plugin Architecture

## Status

Final baseline.

## Purpose

This spec defines Hawk2UI's audio-plugin architecture baseline. Plugin support is first-class and must support high-end editors, generated editors, realtime visual data, stable parameters, host automation, and safe DAW embedding.

## Format Order

Hawk2UI implements plugin formats in this order:

1. CLAP.
2. VST3.
3. AU.
4. LV2 only if Linux strategy requires it.

CLAP is first because it provides a modern lifecycle and strong capabilities for parameter modulation, remote controls, and plugin-host communication. VST3 follows because market adoption requires it.

## Editor Embedding

Plugin editors attach to DAW-owned parent surfaces.

The editor lifecycle must support:

- create editor object,
- receive parent surface handle,
- attach renderer surface,
- report initial logical size,
- receive DPI and resize changes,
- route input where host APIs allow it,
- schedule repaint safely,
- detach and destroy editor surface without process-level quit behavior.

Baseview is the first plugin editor adapter. If Baseview blocks required behavior, Hawk2UI will patch or fork it.

## Parameter Model

The parameter system is modeled heavily from nice-plug.

Parameters must support:

- stable string IDs,
- display names,
- units,
- typed values,
- normalized values,
- default values,
- ranges and distributions,
- stepped and continuous values,
- smoothing,
- flags for hidden, bypass, and non-automatable parameters,
- grouping and nesting,
- generated metadata,
- host display conversion,
- generated editor surfaces.

Stable IDs must not change after release unless a migration explicitly handles the change.

## Automation

Automation uses explicit gesture phases:

- begin gesture,
- change value,
- end gesture.

Host automation updates and UI updates must converge through the parameter model. UI controls do not write directly to DSP state outside the parameter/update contract.

## State And Presets

Plugin state supports:

- parameter state,
- serializable non-parameter state,
- UI-only preferences,
- preset metadata,
- versioned state envelopes,
- migrations,
- host state chunks,
- factory and user preset separation.

State loading must not break audio-thread safety. Expensive migration work happens outside the process callback.

## UI And DSP Boundary

Hawk2UI separates:

- host-automatable parameters,
- UI-only preferences,
- preset state,
- realtime visual data,
- renderer state,
- backend state.

The audio thread never calls JavaScript, rendering code, filesystem APIs, network APIs, or blocking synchronization.

## Realtime Visual Data

Realtime data for meters, scopes, analyzers, waveform trails, and modulation displays uses preallocated non-blocking channels.

The first channel primitive is `rtrb` for single-producer single-consumer audio-thread-to-UI data.

Dropped visual frames are acceptable. Audio-thread stalls are not acceptable.

## Generated Editors

Plugins with metadata but no custom editor must still have a generated parameter editor or diagnostic editor path.

Generated editors must use the same parameter model, automation gestures, validation, and host-safe repaint scheduling as custom editors.

## Test Requirements

Plugin implementation requires tests for:

- parameter metadata generation,
- stable ID validation,
- automation gesture flow,
- state serialization and migration,
- editor attach and teardown,
- realtime visual channel behavior,
- no audio-thread allocation where enforceable,
- CLAP lifecycle behavior,
- VST3 lifecycle behavior when VST3 starts.
