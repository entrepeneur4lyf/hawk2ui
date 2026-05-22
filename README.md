# Hawk2UI

Hawk2UI is a production-focused native UI framework research reboot. The goal is to let developers build desktop applications and embeddable audio-plugin user interfaces from familiar web authoring primitives without shipping Chromium, embedding a WebView, requiring JUCE, or asking application authors to write Rust.

This repository starts clean after the original `hawk2ui` implementation was archived as `hawk2ui-prototypes`. The prototype repo remains prior art for validation, sealed artifacts, QuickJS integration, Baseview experiments, native rendering experiments, and documentation work. This repo is the place for the corrected native-windowing product architecture and production implementation.

## Current Status

Research/specification phase. No runtime code has been accepted into this repo yet.

## Initial Host Targets

- `desktop`: native application windows backed by a Skia rendering pipeline.
- `plugin`: embeddable VST3/CLAP/AU user interfaces with DAW-owned parent-window lifecycles.

## Assets

The Hawk2UI logo assets in `assets/` were carried forward from `hawk2ui-prototypes` and remain part of the production project identity.

## Immediate Gate

Before implementation, this repo must keep the accepted architecture baseline stable and expand individual domain specs from that baseline.

See:

- `docs/specs/0001-product-direction.md`
- `docs/specs/0002-domain-spec-index.md`
- `docs/specs/rendering-architecture.md`
- `docs/technical/crate-selection.md`
- `docs/research/0001-deft-due-diligence.md`
- `docs/decisions/0001-repo-reset.md`
- `docs/decisions/0002-stable-architecture-baseline.md`
