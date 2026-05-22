# Spec 0011: Security

## Status

Final baseline.

## Purpose

This spec defines security requirements for source validation, runtime authority, assets, scripts, secrets, and package trust.

## Trust Boundary Requirements

Hawk2UI must distinguish:

- author source,
- compiled artifacts,
- runtime state,
- host-provided data,
- user-provided data,
- plugin host data,
- secrets,
- assets.

## Source Validation Requirements

Build-time validation must reject:

- unsupported style syntax,
- unsupported script syntax,
- unsafe vector content,
- missing assets,
- undeclared capabilities,
- malformed manifests,
- invalid plugin metadata,
- invalid package targets.

## Script Sandbox Requirements

The script sandbox must deny:

- string-to-code execution,
- undeclared host APIs,
- direct filesystem access,
- direct network access,
- direct process spawning,
- direct native module loading unless explicitly supported by a future capability.

## Asset Security Requirements

Assets must be sanitized or compiled before runtime use.

Asset validation must cover:

- image metadata stripping,
- vector content safety,
- unsupported format rejection,
- declared size limits,
- hash verification.

## Secret Requirements

Secrets must be:

- manifest-declared,
- redacted from diagnostics,
- absent from committed source by default,
- absent from shipped plaintext artifacts unless explicitly intended for public data.

## Acceptance Criteria

- Runtime APIs are capability-scoped.
- Unsafe source fails validation before runtime.
- Assets are trusted compiled records at runtime.
- Secret values are redacted from diagnostics.
