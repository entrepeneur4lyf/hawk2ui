# Spec 0005: Build, Security, And Testing

## Status

Final baseline.

## Build Pipeline

Hawk2UI owns its compiler and packaging pipeline.

The build pipeline is organized as:

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

Parcel is prior art for asset graph, transformer, resolver, cache, and packaging phases. Parcel is not the Hawk2UI build system.

Bun is an external development/build tooling candidate. It is not the embedded runtime.

## Manifest

Human-authored project manifests use TOML.

Generated machine metadata and validation schemas use JSON.

The manifest owns:

- app identity,
- plugin identity,
- package metadata,
- capabilities,
- asset declarations,
- source entrypoints,
- editor metadata,
- plugin parameter metadata when generated outside code,
- preset declarations,
- target declarations.

## Sealed Artifact

A sealed artifact is a versioned container for trusted runtime input.

It contains:

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

Runtime should consume sealed artifacts instead of raw project source where possible.

## Security Model

Hawk2UI uses build-time validation and manifest-scoped runtime authority.

Security rules:

- no ambient filesystem access,
- no ambient network access,
- no undeclared clipboard access,
- no undeclared secrets,
- no string-to-code execution path,
- no unsafe vector asset content,
- no runtime trust in unvalidated source files,
- no host API access outside declared capabilities.

Script runtime policy denies `eval`, `Function`, and equivalent string-to-code paths.

Assets are sanitized or compiled before runtime use. Secrets are manifest-declared and redacted from diagnostics.

## Dependency Policy

Dependencies may be accepted when they solve the product problem, even if their upstream marks them alpha or experimental. Hawk2UI handles API changes when they occur.

Every dependency added to implementation must have:

- crate name,
- version requirement,
- enabled features,
- reason for use,
- license check,
- security check,
- platform implications.

Required dependency checks once the workspace exists:

```bash
cargo audit
cargo deny check
cargo machete
cargo test --workspace
```

## Test Strategy

Testing follows implemented behavior. Hawk2UI does not block implementation on a speculative full compatibility matrix.

Required test gates:

- unit tests for parser, style, layout, scene, renderer, runtime, manifest, and plugin primitives,
- integration tests for source-to-artifact and artifact-to-render paths,
- visual regression tests for rendering quality,
- host lifecycle tests for create, resize, DPI, repaint, close, and teardown,
- plugin lifecycle tests for editor attach, repaint, parameter updates, automation, state, and teardown,
- security rejection tests for invalid source, denied APIs, unsafe assets, and undeclared capabilities,
- performance benchmarks for startup, style, layout, rendering, script execution, and plugin visual update paths.

## Implementation Readiness

Implementation may begin when these final baselines are accepted:

- product and scope,
- architecture decisions,
- rendering,
- plugin architecture,
- build/security/testing.

The first code slice should prove a real rendering path:

- Rust workspace scaffold,
- typed style property subset,
- Taffy layout wrapper,
- retained scene graph,
- Skia CPU render path,
- headless visual regression fixture.
