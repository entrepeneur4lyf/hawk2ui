# Spec 0001: Product And Scope

## Status

Final baseline.

## Purpose

This spec defines the product scope and externally visible product requirements for Hawk2UI.

## Product Definition

Hawk2UI is a native UI framework for building desktop applications and embeddable audio-plugin editors from familiar web-era authoring primitives without requiring application authors to write Rust.

The product has two first-class host targets:

- desktop applications with owned native windows,
- audio-plugin editors embedded in DAW-owned plugin surfaces.

Both host targets use one coherent UI model for structure, styling, rendering, assets, runtime state, and developer tooling.

## Product Requirements

Hawk2UI must provide:

- native desktop application surfaces,
- embeddable audio-plugin editor surfaces,
- declarative UI structure,
- typed style and layout processing,
- JavaScript or TypeScript application logic,
- asset compilation,
- package manifests,
- capability-scoped runtime APIs,
- visual regression tooling,
- developer diagnostics,
- user-facing manual documentation.

## Product Constraints

Hawk2UI must not require:

- Chromium,
- a WebView as the primary runtime,
- Electron or Tauri architecture,
- JUCE,
- Rust UI code from application authors,
- browser API parity,
- ambient runtime access to host resources.

## Visual Quality Requirement

Hawk2UI must support premium, distinctive interfaces for creative tools, audio plugins, dashboards, internal tools, and desktop applications.

The product must support:

- custom controls,
- graph surfaces,
- animated visual feedback,
- high-DPI text and vector rendering,
- gradients and effects,
- image and vector asset layers,
- theming and design tokens,
- dense expert panels,
- user-editable visual preferences.

Default controls are starter skins and examples. They are not the visual ceiling of the product.

## Component Requirement

Built-in components must be headless by default.

Headless components provide:

- behavior,
- input semantics,
- accessibility hooks,
- layout contracts,
- state machines,
- efficient render hooks.

Visual form comes from styles, themes, assets, and custom draw surfaces.

## Acceptance Criteria

- Desktop applications and plugin editors are both represented in the product model.
- Product requirements do not depend on browser rendering.
- Product requirements do not require Rust UI code from app authors.
- Visual quality requirements include custom controls, assets, vector drawing, graph surfaces, and animation.
