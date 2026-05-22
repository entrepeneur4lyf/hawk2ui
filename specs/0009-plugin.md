# Spec 0009: Plugin

## Status

Final baseline.

## Purpose

This spec defines audio-plugin requirements for format support, editor embedding, parameters, automation, state, and realtime visual data.

## Format Requirements

Plugin support must include:

- CLAP format support,
- VST3 format support,
- AU format support for macOS distribution,
- standalone wrapper support,
- generated plugin metadata,
- package bundle output.

## Editor Requirements

Plugin editors must support:

- DAW-owned parent surface attachment,
- editor create and destroy lifecycle,
- initial logical size reporting,
- DPI updates,
- host-driven resizing,
- safe repaint scheduling,
- generated editor fallback,
- custom editor rendering.

## Parameter Requirements

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
- hidden, bypass, and non-automatable flags,
- grouping and nesting,
- generated metadata,
- host display conversion.

## Automation Requirements

Automation must support:

- begin gesture,
- value change,
- end gesture,
- host-originated updates,
- UI-originated updates,
- generated editor binding,
- custom editor binding.

## State Requirements

Plugin state must support:

- parameter state,
- serializable non-parameter state,
- UI-only preferences,
- preset metadata,
- versioned state envelopes,
- migrations,
- host state chunks,
- factory and user preset separation.

## Realtime Visual Data Requirements

Realtime visual data must support:

- preallocated transport from audio processing to UI,
- non-blocking writes from the audio thread,
- meter streams,
- analyzer streams,
- scope streams,
- modulation streams,
- acceptable frame drops without audio stalls.

## Acceptance Criteria

- Plugin editor attachment does not require top-level window ownership.
- Parameter IDs are stable and validated.
- Automation gestures are explicit.
- State is versioned and migratable.
- Audio-thread-to-UI visual data is non-blocking.
