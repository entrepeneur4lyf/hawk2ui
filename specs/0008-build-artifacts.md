# Spec 0008: Build And Artifacts

## Status

Final baseline.

## Purpose

This spec defines build pipeline, manifest, asset, and sealed artifact requirements.

## Build Pipeline Requirements

The build pipeline must perform:

1. source discovery,
2. manifest validation,
3. asset discovery,
4. source validation,
5. style compilation,
6. script compilation,
7. asset compilation,
8. scene/style/layout artifact generation,
9. sealed artifact packaging,
10. native app or plugin bundle packaging,
11. verification report generation.

## Manifest Requirements

The manifest must declare:

- app identity,
- plugin identity where applicable,
- package metadata,
- capabilities,
- asset declarations,
- source entrypoints,
- editor metadata,
- plugin parameter metadata where applicable,
- preset declarations,
- target declarations.

## Artifact Requirements

A sealed artifact must include:

- schema version,
- manifest snapshot,
- compiled scripts,
- compiled styles,
- asset manifest,
- compiled assets,
- capability declarations,
- hashes,
- build metadata,
- target metadata.

## Validation Requirements

Build output must include diagnostics for:

- invalid manifests,
- unsupported style source,
- unsupported script source,
- unsafe assets,
- missing assets,
- undeclared capabilities,
- target incompatibility.

## Acceptance Criteria

- Runtime can consume sealed artifacts without raw source parsing.
- Artifact records are versioned and hash-addressed.
- Build diagnostics identify the source file and failing rule.
