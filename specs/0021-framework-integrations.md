# Spec 0021: Framework Integrations

## Status

Final baseline.

## Purpose

This spec defines framework integration requirements for Hawk2UI authoring from JavaScript and TypeScript UI ecosystems.

## Framework Support Requirements

Hawk2UI must support integrations for:

- Svelte 5,
- React 19 and later,
- Vue 3.5 and later,
- Solid,
- direct Hawk2UI native element authoring.

Each integration must emit Hawk2UI typed records rather than browser-owned DOM records.

## Integration Requirements

Framework integrations must support:

- component creation,
- component update,
- component teardown,
- keyed children,
- properties,
- event bindings,
- references,
- style references,
- asset references,
- lifecycle hooks,
- batched updates,
- diagnostics.

## Runtime Requirements

Framework runtime integration must not require:

- browser event objects,
- browser layout APIs,
- browser rendering APIs,
- ambient document ownership,
- host lifecycle ownership.

## Build Requirements

Framework builds must provide:

- TypeScript compilation,
- framework transform execution,
- source maps where available,
- diagnostics with source locations,
- production bundle output,
- development bundle output,
- sealed artifact integration.

## Acceptance Criteria

- Svelte, React, Vue, Solid, and direct native authoring can emit Hawk2UI records.
- Framework output can drive style, layout, runtime, and rendering without browser ownership.
- Framework diagnostics point to author source files.
- Framework examples build through the public toolchain.
