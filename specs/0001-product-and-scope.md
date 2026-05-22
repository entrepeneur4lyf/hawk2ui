# Spec 0001: Product And Scope

## Status

Final baseline.

## Product Definition

Hawk2UI is a native UI framework for building desktop applications and embeddable audio-plugin editors from familiar web-era authoring primitives without shipping Chromium, embedding a WebView, requiring JUCE, or requiring application authors to write Rust.

The product has two first-class host targets:

- desktop applications with owned native windows,
- audio-plugin editors embedded in DAW-owned plugin surfaces.

The plugin target is not a separate product. It is a hard engineering proof point for the same rendering, style, component, asset, and runtime model used by desktop applications.

## Non-Negotiables

- No Chromium dependency.
- No WebView dependency as the primary runtime.
- No Electron or Tauri architecture.
- No JUCE dependency.
- No Rust requirement for app or plugin UI authors.
- No hidden browser compatibility promise.
- No audio-thread blocking in plugin use cases.
- No ambient runtime authority; host APIs are capability-scoped.
- No architecture that only works for top-level app windows.

## Authoring Model

Hawk2UI uses a web-familiar authoring model without promising browser parity.

Author input consists of:

- declarative UI structure,
- a documented native style subset,
- JavaScript or TypeScript logic,
- design assets,
- manifest-declared capabilities and package metadata.

Framework compatibility is valuable, but no third-party framework owns the product model. The first authoring target is Hawk2UI's native element/custom renderer model. Svelte 5 is the first named framework proof target after that. React, Vue, and Solid follow after the native model is stable.

## Visual Quality Requirement

Visual quality is a core product requirement. Hawk2UI must support premium, distinctive interfaces for creative tools, audio plugins, dashboards, internal tools, and desktop applications.

The system must support:

- custom controls,
- graph surfaces,
- animated visual feedback,
- high-DPI text and vector rendering,
- gradients and effects,
- image and vector asset layers,
- theming and design tokens,
- dense expert panels,
- user-editable visual preferences.

Default controls may exist, but they are starter skins and examples, not the visual ceiling.

## Built-In Components

Rust-provided components are headless by default. They provide behavior, input semantics, accessibility hooks, layout contracts, state machines, and efficient render hooks. Their visual form comes from styles, themes, assets, and custom draw surfaces.

## Required Product Surfaces

Hawk2UI must eventually provide:

- desktop app scaffolding,
- plugin editor scaffolding,
- manifest validation,
- asset compilation,
- style compilation,
- native package/sealed artifact output,
- plugin bundle output,
- visual regression tooling,
- developer diagnostics,
- user-facing manual documentation.

## Out Of Scope For The Baseline

The baseline does not include browser API parity, general web content rendering, arbitrary remote document rendering, or a WebView fallback path as a primary product feature.
