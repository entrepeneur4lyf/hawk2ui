# Spec 0018: Smoke Apps And Fixtures

## Status

Final baseline.

## Purpose

This spec defines smoke application and fixture requirements for proving Hawk2UI works as a complete product across desktop apps, plugin editors, rendering, styling, runtime, assets, security, and packaging.

## Smoke App Requirements

The project must include runnable smoke applications for:

- a complete desktop application,
- a dense dashboard application,
- a plugin editor with generated controls,
- a plugin editor with custom controls,
- a realtime meter and analyzer surface,
- a style and asset gallery,
- a security denial fixture set.

Smoke applications must use the same public authoring and build pipeline exposed to product users.

## Coverage Requirements

Smoke fixtures must cover:

- manifest validation,
- source compilation,
- style compilation,
- asset compilation,
- sealed artifact generation,
- runtime startup,
- scene creation,
- first frame export,
- native surface lifecycle,
- input events,
- state updates,
- diagnostics,
- package verification.

## Visual Fixture Requirements

Visual fixtures must include deterministic coverage for:

- text,
- shapes,
- gradients,
- image layers,
- vector layers,
- shadows,
- transforms,
- custom controls,
- graph surfaces,
- high-DPI scaling.

## Plugin Fixture Requirements

Plugin fixtures must cover:

- editor create and destroy,
- parent surface attachment,
- resize,
- DPI changes,
- parameter updates,
- automation gestures,
- state save and restore,
- preset loading,
- realtime visual data.

## Acceptance Criteria

- Smoke apps build through the public toolchain.
- Smoke apps run through automated verification.
- Smoke apps exercise desktop and plugin surfaces.
- Security denial fixtures fail before runtime launch.
- Visual smoke output is deterministic for fixed fixtures.
