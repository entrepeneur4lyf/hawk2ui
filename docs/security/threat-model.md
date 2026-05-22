# Hawk2UI Threat Model

Hawk2UI treats source input, compiled artifacts, runtime state, host-provided data, user-provided data, plugin host data, secrets, and assets as separate trust boundaries.

## Release-Blocking Threats

The canonical machine-readable registry lives in `security/threat-model.toml` and covers:

- malicious source,
- malformed artifacts,
- unsafe assets,
- hostile package input,
- malicious plugin host data,
- untrusted user data,
- secret exposure,
- denied platform authority.

## Capability Boundaries

Capability rejection cases live in `security/rejection-cases.toml`. Every capability key must have one allow case and one deny case.

Covered capabilities are filesystem, network, clipboard, secrets, database, package targets, plugin metadata, host APIs, and runtime bindings.

## Source And Asset Fixtures

Source and asset attack fixtures live in `security/source-asset-fixtures.toml` and map every malicious fixture to a stable diagnostic rule.

## Runtime Authority

The runtime sandbox denies string-to-code execution, undeclared host APIs, direct filesystem access, direct network access, process spawning, and native module loading.

Diagnostics must redact secrets and executable source payloads before they leave the runtime boundary.

## Package Trust

Package trust validation requires artifact schema version match, manifest snapshot hash, compiled asset hashes, compiled script hashes, target metadata, verified signature status, and a present verification report.
