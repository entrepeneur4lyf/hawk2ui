# Hawk2UI Release Checklist

Run these commands from the repository root before tagging or publishing a release candidate.

## Required Commands

- `rtk bash scripts/release-check.sh --version-only`
- `rtk bash scripts/release-check.sh --packages-only`
- `rtk bash scripts/release-check.sh --changelog-only`
- `rtk bash scripts/release-check.sh`

## Required Evidence

- Confirm release criteria evidence files are written under `target/release-evidence/`.
- Confirm package target evidence covers Windows, macOS, Linux Wayland, Linux X11, release-backed CLAP/VST3/AU plugin bundles, sealed artifacts, debug packages, and release packages.
- Confirm React and Vue Deno runtime evidence covers package API checks, sealed JS module graph execution, desktop smoke evidence, and plugin smoke evidence.
- Confirm CI/release jobs restore native binary caches for rusty_v8 archives and rust-skia `skia-binaries` output before running release gates.
- Confirm runtime bundle evidence records sealed JS module source-map hashes, dependency origins, lockfile hash, graph entrypoint, static imports, dynamic imports, and chunk membership.
- Confirm changelog verification evidence is linked before tagging.
- Confirm `CHANGELOG.md` includes Added, Changed, Fixed, Security, Compatibility, Migration, and Known Limitations sections.

## Blocking Rule

Any failing release criterion, version policy mismatch, package target validation failure, changelog validation failure, CI-equivalent check failure, dependency policy failure, documentation build failure, or security gate failure blocks release.
