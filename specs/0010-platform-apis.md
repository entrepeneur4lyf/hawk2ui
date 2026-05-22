# Spec 0010: Platform APIs

## Status

Final baseline.

## Purpose

This spec defines capability-scoped platform API requirements.

## Capability Requirements

Every platform API must declare:

- manifest capability key,
- allowed operations,
- denied operations,
- input schema,
- output schema,
- error schema,
- runtime availability,
- desktop applicability,
- plugin applicability.

## Filesystem Requirements

Filesystem APIs must support scoped access only.

Filesystem access must distinguish:

- project assets,
- app data,
- cache data,
- user-selected files,
- plugin preset storage,
- forbidden paths.

## Network Requirements

Network APIs must require manifest declarations.

Network access must support allowlists, structured errors, and diagnostics for denied access.

## Clipboard Requirements

Clipboard APIs must require manifest declarations and must expose explicit data-type support.

## Secrets Requirements

Secrets must be declared in the manifest and redacted from diagnostics.

Runtime secret sources must be explicit and must not require plaintext developer secrets in shipped artifacts.

## Database Requirements

Database APIs must be capability-scoped and must support migrations, transactions, and safe storage paths.

## Acceptance Criteria

- No platform API is available without declared capability.
- Denied access produces structured diagnostics.
- Plugin contexts expose only host-safe API subsets.
