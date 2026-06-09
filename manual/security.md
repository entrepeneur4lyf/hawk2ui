# Hawk2UI Security

Hawk2UI projects declare capabilities explicitly and must fail closed when source, assets, manifests, or host API requests violate the supported contract.

## Capability Model

Canonical `hawk.json` capabilities are declared in `permissions.capabilities` as string keys. Legacy `manifest.hawk.toml` maps `[capabilities].keys` into the same validated capability list during migration and compatibility parsing. Runtime code should consume declared capabilities through `CapabilityKey` and host binding records, not ambient process authority.

## Security Fixtures

The repository includes security-denial fixtures that define the expected rejection classes. These paths are part of the manual source-of-truth coverage:

- `fixtures/security/unsupported-style.css`
- `fixtures/security/unsupported-script.ts`
- `fixtures/security/unsafe-vector.svg`
- `fixtures/security/oversized-asset.manifest`
- `fixtures/security/hash-mismatch.manifest`
- `fixtures/security/missing-asset.manifest`
- `fixtures/security/malformed-manifest.toml`

The example manifest `examples/security-denials/hawk.json` exercises this domain from a user project shape.

## Release Key Management

Release artifacts are signed before distribution and verified by the CLI before release. Packaged desktop launchers verify the runtime descriptor artifact signature against trusted release keys before loading. Treat signing keys as release infrastructure secrets, not project source.

Required release commands and environment variables:

- `build-release` requires `HAWK2UI_RELEASE_SIGNING_KEY_ID` and `HAWK2UI_RELEASE_SIGNING_KEY_HEX`.
- `package-desktop` requires `HAWK2UI_RELEASE_SIGNING_KEY_ID` and `HAWK2UI_RELEASE_SIGNING_KEY_HEX`.
- `package-plugin` requires `HAWK2UI_RELEASE_SIGNING_KEY_ID` and `HAWK2UI_RELEASE_SIGNING_KEY_HEX`.
- `verify-artifact` and packaged desktop launchers require trusted release keys through `HAWK2UI_TRUSTED_RELEASE_KEYS`.

`HAWK2UI_RELEASE_SIGNING_KEY_HEX` is a 64-hex-byte Ed25519 private signing key. Keep it outside the repository, inject it through the release environment, rotate it when access changes, and never ship it in a sealed artifact. `HAWK2UI_TRUSTED_RELEASE_KEYS` is a comma-separated trust list of `key-id:64-hex-public-key` entries used by `hawk2ui verify-artifact` and packaged desktop runtime loading.

The release workflow is:

```bash
HAWK2UI_RELEASE_SIGNING_KEY_ID=local-release \
HAWK2UI_RELEASE_SIGNING_KEY_HEX=<64-hex-private-key> \
hawk2ui build-release

HAWK2UI_RELEASE_SIGNING_KEY_ID=local-release \
HAWK2UI_RELEASE_SIGNING_KEY_HEX=<64-hex-private-key> \
hawk2ui package-desktop

HAWK2UI_RELEASE_SIGNING_KEY_ID=local-release \
HAWK2UI_RELEASE_SIGNING_KEY_HEX=<64-hex-private-key> \
hawk2ui package-plugin

HAWK2UI_TRUSTED_RELEASE_KEYS=local-release:<64-hex-public-key> \
hawk2ui verify-artifact
```

Build and package commands fail closed without signing material. Verification fails closed when the artifact is unsigned, signed by an untrusted key, has malformed signature metadata, or does not match its signed payload.

## Security Evidence Vocabulary

`hawk2ui-security` is an evidence vocabulary crate. It records security decisions that concrete validators have already made; it is not a parallel enforcement engine. Enforcement lives in the owning production crates:

- `hawk2ui-build` validates manifests, release signing, sealed artifacts, package trust, and target metadata.
- `hawk2ui-assets` validates asset sizes, hashes, image metadata, and SVG/vector safety.
- `hawk2ui-script` enforces script sandboxing, host-call policy, deterministic timers, and execution limits.
- `hawk2ui-platform` enforces scoped filesystem, network, clipboard, secret-store, AI provider, audio cue, dialog/file picker, localization, MCP tool, notification, global shortcut, and database access.
- `hawk2ui-security-model` validates threat registries, capability rejection fixtures, runtime authority, secret redaction, and package trust records.

Evidence records should be emitted only after those concrete validators accept or reject an operation. Do not construct evidence records as a substitute for running the validator that owns the domain.

## Author Requirements

- Declare required capabilities in the manifest before using host services.
- Treat validation and verification diagnostics as release blockers when they have error severity.
- Keep asset paths resolvable and hashes stable before sealing artifacts.
- Avoid unsupported source and style features; they should produce deterministic diagnostics.
- Do not expect plugin hosts or desktop adapters to provide undeclared filesystem, network, clipboard, database, secret, AI, audio, dialog, localization, MCP, notification, shortcut, or host API access.
