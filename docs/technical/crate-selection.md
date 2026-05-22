# Technical Document: Crate Selection Baseline

## Purpose

This document records the initial Rust crate candidates for Hawk2UI and the version status observed during the production reboot planning phase.

The repo currently has no accepted runtime code, so this is not a `Cargo.lock` audit. It is the dependency baseline for future `Cargo.toml` work. Before adding any crate to the workspace, re-run the verification command in the dependency policy section and update this file if the published version has changed.

## Version Audit Date

- Date: 2026-05-22
- Source: crates.io through `cargo search` and `cargo info`
- Policy: prefer the latest stable release. Use release candidates, beta releases, alpha releases, or experimental versions only when explicitly justified in the relevant domain spec.

## Dependency Policy

Every implementation spec that adds dependencies must include:

- exact crate names,
- chosen version requirement,
- reason for choosing the crate,
- feature flags to enable and disable,
- platform-specific build implications,
- license compatibility check,
- maintenance risk assessment,
- replacement/fallback plan for high-risk crates.

Freshness verification before dependency changes:

```bash
cargo search <crate-name> --limit 3
cargo info <crate-name>
```

Workspace dependency hygiene once code exists:

```bash
cargo update
cargo outdated --workspace
cargo audit
cargo deny check
cargo machete
cargo nextest run --workspace
```

Planned tooling crates:

| Tool | Observed Version | Use |
|------|------------------|-----|
| `cargo-deny` | `0.19.7` | License, advisory, ban, and source checks. |
| `cargo-audit` | `0.22.1` | RustSec vulnerability checks. |
| `cargo-outdated` | `0.19.0` | Dependency freshness checks. |
| `cargo-machete` | `0.9.2` | Unused dependency detection. |
| `cargo-nextest` | `0.9.136` | Faster workspace test execution. |

## Core Rendering And UI Pipeline

| Domain | Crate | Observed Version | Decision | Notes |
|--------|-------|------------------|----------|-------|
| 2D renderer | `skia-safe` | `0.97.0` | preferred | Main 2D renderer candidate. Hide behind Hawk2UI renderer traits. Expect build-time cost and backend feature decisions. |
| Layout | `taffy` | `0.10.0` stable, latest `0.11.0-experimental-cache-fix.3` | preferred stable | Use latest stable unless a spec accepts the experimental cache-fix line. Prototype used `0.10`. |
| CSS parser/transformer | `lightningcss` | `1.0.0-alpha.71` | preferred | Latest observed line is alpha. Acceptable only with explicit CSS spec coverage and upgrade tests. Prototype used older alpha `1.0.0-alpha.57`. |
| Text layout | `parley` | `0.9.0` | preferred | Rich text layout candidate. Prototype already had Parley-backed shaping. |
| Font database | `fontdb` | `0.23.0` | likely | Font discovery/loading support. Confirm interaction with Parley/Skia before pinning. |
| Shaping/raster support | `swash` | `0.2.7` | likely | Useful with modern Rust text stack; confirm exact role after text spec. |
| Accessibility | `accesskit` | `0.24.0` | preferred | Cross-platform accessibility tree candidate. Needs host adapter compatibility specs. |
| Window-handle interop | `raw-window-handle` | `0.6.2` | preferred | Required boundary for host/render surface interop. |

## Host And Windowing

| Domain | Crate | Observed Version | Decision | Notes |
|--------|-------|------------------|----------|-------|
| Desktop windows | `winit` | `0.30.12` stable, latest `0.31.0-beta.2` | candidate | Use stable line unless host spec accepts beta. Strong desktop candidate. |
| Plugin-oriented windowing | `baseview` | `0.1.1` | candidate/high-risk | Latest published version is old but still relevant for parented plugin surfaces. Treat as adapter candidate, not runtime owner. Fork risk is real. |
| Desktop/test harness | `sdl3` | `0.18.4` | candidate | Useful as desktop backend or test harness. Does not solve DAW-owned plugin embedding by itself. |
| File watching | `notify` | `8.2.0` stable, latest `9.0.0-rc.4` | preferred stable | Use stable line for development server unless rc is explicitly accepted. |

## JavaScript Runtime Candidates

| Domain | Crate | Observed Version | Decision | Notes |
|--------|-------|------------------|----------|-------|
| Rust-native JS runtime | `boa_engine` | `0.21.1` | first spike | Preferred first embedded runtime spike. Evaluate modules, promises, interruption, host bindings, memory limits, and framework workloads. |
| V8 runtime framework | `deno_core` | `0.401.0` | comparison spike | Strong compatibility candidate but carries V8/toolchain weight. User has prior embedded V8 experience. |
| V8 bindings | `v8` | `149.0.0` | comparison dependency | Lower-level V8 option. Likely through `deno_core` unless a spec justifies direct use. |
| JavaScriptCore bindings | `javascriptcore-rs` | `1.1.2` | comparison spike | Platform and packaging implications must be checked before serious adoption. |
| QuickJS bindings | `rquickjs` | `0.11.0` | fallback/comparison | Prototype used QuickJS concepts. Keep as comparison, not default. |

## Assets, Images, And Vector Input

| Domain | Crate | Observed Version | Decision | Notes |
|--------|-------|------------------|----------|-------|
| General image IO | `image` | `0.25.10` | preferred | Decode/encode foundation for common formats. |
| WebP | `image-webp` | `0.2.4` | candidate | Dedicated WebP support if `image` alone is insufficient for build pipeline goals. |
| AVIF encoding | `ravif` | `0.13.0` | candidate | Candidate for optimized AVIF output. Validate quality/speed tradeoffs. |
| PNG optimization | `oxipng` | `10.1.1` | preferred tooling | Build-time PNG optimization. Runtime should consume compiled assets. |
| SVG simplification | `usvg` | `0.47.0` | preferred | SVG sanitization/simplification candidate. Must be wrapped by strict asset policy. |
| SVG rendering | `resvg` | `0.47.0` | preferred | SVG raster/vector conversion candidate after sanitization. |

## Data, Manifests, And Schemas

| Domain | Crate | Observed Version | Decision | Notes |
|--------|-------|------------------|----------|-------|
| Serialization | `serde` | `1.0.228` | preferred | Standard data model serialization. |
| JSON | `serde_json` | `1.0.150` | preferred | Artifact metadata, diagnostics, schema testing. |
| TOML | `toml` | `1.1.2+spec-1.1.0` | preferred | Human-authored manifests if TOML is chosen. |
| TOML editing | `toml_edit` | `0.25.11+spec-1.1.0` | candidate | Useful for scaffold/update commands that preserve formatting. |
| JSON Schema generation | `schemars` | `1.2.1` | preferred | Generate schema for manifests and config files. |
| JSON Schema validation | `jsonschema` | `0.46.5` | preferred | Validate manifests/configs in CLI and tests. |
| Cargo metadata | `cargo_metadata` | `0.23.1` | tooling | Workspace inspection and build diagnostics. |
| UTF-8 paths | `camino` | `1.2.2` | preferred | Stable internal path type for project files. |
| Temp UTF-8 paths | `camino-tempfile` | `1.4.1` | test tooling | Temp project tests and artifact tests. |
| Platform directories | `directories` | `6.0.0` | preferred | OS-appropriate config/cache/data paths. |

## CLI, Diagnostics, And Infrastructure

| Domain | Crate | Observed Version | Decision | Notes |
|--------|-------|------------------|----------|-------|
| CLI parser | `clap` | `4.6.1` | preferred | Main command-line interface parser. Distinct from CLAP audio plugin format. |
| Structured logging | `tracing` | `0.1.44` | preferred | Runtime, build, and host diagnostics. |
| Trace subscribers | `tracing-subscriber` | `0.3.23` | preferred | CLI/dev logging setup. |
| Library errors | `thiserror` | `2.0.18` | preferred | Public and internal typed errors. |
| Application errors | `anyhow` | `1.0.102` | preferred | CLI top-level error aggregation only. Avoid leaking into library APIs. |
| Async runtime | `tokio` | `1.52.3` | preferred | Default async runtime candidate for CLI/dev/runtime host services. |
| Small async runtime | `smol` | `2.0.2` | spike-only | Useful for Boa examples/comparison. Do not mix runtimes without runtime architecture spec. |

## Realtime, Audio, And Plugin Infrastructure

| Domain | Crate | Observed Version | Decision | Notes |
|--------|-------|------------------|----------|-------|
| Realtime SPSC ring buffer | `rtrb` | `0.3.4` | preferred | Strong candidate for audio-thread-to-UI visual data. |
| General ring buffer | `ringbuf` | `0.5.0` | candidate | Compare against `rtrb` for realtime constraints. |
| General channels | `crossbeam-channel` | `0.5.15` | preferred non-realtime | Useful off the audio thread. Not a realtime audio-thread primitive by default. |
| VST3 bindings | `vst3` | `0.3.0` | candidate | Direct VST3 adapter candidate. Compare with framework approaches. |
| CLAP bindings | `clap-sys` | `0.5.0` | candidate | Raw CLAP binding candidate. Also evaluate `clack-*` ecosystem during plugin spec. |
| NIH-plug core | `nih_plug_core` | `0.1.2` | prior art / possible interop | Good prior art; do not assume as foundation until plugin strategy spec. |
| Audio IO | `cpal` | `0.17.3` | standalone candidate | Useful for standalone plugin wrapper and app audio. |
| Audio decode | `symphonia` | `0.6.0` | preferred | App audio playback and asset pipeline candidate. |
| MIDI | `midir` | `0.11.0` | preferred candidate | MIDI learn/external control support. |

## Testing And Quality Crates

| Domain | Crate | Observed Version | Decision | Notes |
|--------|-------|------------------|----------|-------|
| Snapshot testing | `insta` | `1.47.2` | preferred | Diagnostics, artifact metadata, and visual/regression metadata snapshots. |
| Property tests | `proptest` | `1.11.0` | preferred | Parser, layout, and manifest validation properties. |
| Fuzz data generation | `arbitrary` | `1.4.2` | candidate | Fuzzing and structured random inputs. |
| Benchmarks | `criterion` | `0.8.2` | preferred | Style, layout, render, and runtime benchmarks. |

## Known Freshness Risks

- `baseview` is still the only obvious parented plugin-window candidate in this stack, but the published version is `0.1.1`. Treat it as high-risk until the Baseview adapter spec proves lifecycle correctness or selects a fork/replacement.
- `lightningcss` is still on an alpha version line even though it is the preferred CSS parser. The CSS spec must pin exact behavior with tests so upgrades are safe.
- `taffy`, `winit`, and `notify` currently expose newer prerelease lines. Use the latest stable line unless a domain spec explicitly accepts prerelease risk.
- Plugin crates are fragmented. `vst3`, `clap-sys`, `nih_plug_core`, and the `clack-*` ecosystem need a dedicated plugin format strategy spec before choosing foundations.
- JavaScript runtime selection is unresolved. Boa is the first spike, not a locked product dependency.

## Required Follow-Up Specs

- `docs/specs/javascript-runtime-choice.md`
- `docs/specs/host-abstraction.md`
- `docs/specs/baseview-adapter.md`
- `docs/specs/skia-renderer-abstraction.md`
- `docs/specs/style-system.md`
- `docs/specs/layout-architecture.md`
- `docs/specs/plugin-format-strategy.md`
- `docs/specs/security-model.md`
- `docs/specs/test-strategy.md`
