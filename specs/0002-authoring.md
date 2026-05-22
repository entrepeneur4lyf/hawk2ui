# Spec 0002: Authoring

## Status

Final baseline.

## Purpose

This spec defines the source authoring model exposed to Hawk2UI application and plugin UI authors.

## Source Inputs

A Hawk2UI project contains:

- a project manifest,
- declarative UI source,
- style source,
- JavaScript or TypeScript source,
- image assets,
- vector assets,
- font assets,
- design token files,
- optional plugin metadata,
- optional preset metadata.

## UI Structure Requirements

The UI structure model must support:

- elements,
- components,
- stable node identity,
- child ordering,
- keyed dynamic children,
- properties,
- event handlers,
- references,
- custom controls,
- custom draw surfaces.

The source structure must compile into typed runtime records before rendering.

## Framework Requirements

Hawk2UI must expose a native element/custom renderer model.

Framework integrations must map into that model without requiring browser DOM ownership. Framework output must be able to produce typed structure, event bindings, style references, and asset references consumed by Hawk2UI.

## Event Requirements

The authoring model must support:

- pointer events,
- keyboard events,
- focus events,
- input events,
- resize events,
- lifecycle events,
- custom component events,
- plugin parameter events.

Event delivery must not require browser event objects.

## State Requirements

The authoring model must support:

- component state,
- app state,
- UI-only preferences,
- plugin parameter binding,
- runtime subscriptions,
- batched updates,
- deterministic teardown.

## Acceptance Criteria

- Author input can be compiled into typed UI records.
- Framework integrations do not own the renderer or host lifecycle.
- Event and state APIs are native Hawk2UI concepts, not browser object requirements.
