# Hawk2UI Security

Hawk2UI projects declare capabilities explicitly and must fail closed when source, assets, manifests, or host API requests violate the supported contract.

## Capability Model

Manifest capabilities are declared in `[capabilities]` as string keys. Runtime code should consume declared capabilities through `CapabilityKey` and host binding records, not ambient process authority.

## Security Fixtures

The repository includes security-denial fixtures that define the expected rejection classes. These paths are part of the manual source-of-truth coverage:

- `fixtures/security/unsupported-style.css`
- `fixtures/security/unsupported-script.ts`
- `fixtures/security/unsafe-vector.svg`
- `fixtures/security/oversized-asset.manifest`
- `fixtures/security/hash-mismatch.manifest`
- `fixtures/security/missing-asset.manifest`
- `fixtures/security/malformed-manifest.toml`

The example manifest `examples/security-denials/manifest.hawk.toml` exercises this domain from a user project shape.

## Author Requirements

- Declare required capabilities in the manifest before using host services.
- Treat validation and verification diagnostics as release blockers when they have error severity.
- Keep asset paths resolvable and hashes stable before sealing artifacts.
- Avoid unsupported source and style features; they should produce deterministic diagnostics.
- Do not expect plugin hosts or desktop adapters to provide undeclared filesystem, network, clipboard, database, secret, or host API access.
