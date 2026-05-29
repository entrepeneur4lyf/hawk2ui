<table>
<tr>
<td width="400">
<img src="assets/hawk2ui.webp" alt="Hawk2UI" width="400">
</td>
<td valign="top">

# Hawk2UI

**Build desktop apps and audio-plugin editors with TypeScript, CSS, and a manifest.**
**No Chromium. No WebView. No JUCE. No Rust required.**

Hawk2UI compiles familiar web authoring primitives into a signed native artifact, rendered by a pure-Rust engine — for both standalone desktop windows and editors embedded inside a DAW.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![Targets](https://img.shields.io/badge/targets-desktop%20%C2%B7%20VST3%20%C2%B7%20CLAP%20%C2%B7%20AU-blueviolet.svg)]()

**Native UI from web primitives — without the browser.**

</td>
</tr>
</table>

---

```bash
cargo run -p hawk2ui-cli -- new            # scaffold a project
cargo run -p hawk2ui-cli -- dev            # hot-reload the native surface
cargo run -p hawk2ui-cli -- build-release  # produce a sealed artifact
```

## How It Works

A project is a `manifest.hawk.toml` plus a TypeScript entry point, CSS, and assets. The build pipeline validates and compiles those into a signed, schema-versioned **sealed artifact**, which a host loads and renders. One engine drives two host surfaces:

- **`desktop`** — native OS application windows (Hawk2UI owns the window lifecycle), via `winit`.
- **`plugin`** — editor surfaces embedded in a DAW-owned parent window, via `baseview`, packaged as CLAP/VST3/AU/standalone.

The native stack: `skia-safe` rendering, `taffy` flexbox/grid layout, `parley`/`swash`/`fontdb` text shaping, `boa_engine` + `oxc` for JavaScript/TypeScript, and `accesskit` accessibility. `unsafe` is forbidden workspace-wide except at the plugin window-handle FFI boundary.

## Status

Implemented on a stable architecture baseline (`docs/decisions/0002-stable-architecture-baseline.md`) with enforced release-readiness gates. The core engine, runtime, desktop and plugin hosts, framework adapters, and the build/CLI toolchain are in place and covered by CI (format, clippy, unit + integration + smoke tests, render benchmark, docs, dependency policy). Full application and plugin packaging is still being completed — see `CHANGELOG.md` for current limitations.

## Build & Test

Cargo workspace, Rust edition 2024 (MSRV 1.95). Two gates mirror CI:

```bash
cargo run -p xtask -- check-fast   # before every commit: fmt, check, tests, contract/template filters, smoke apps
cargo run -p xtask -- check        # full gate: + clippy -D warnings, integration, bench, docs, cargo-deny
```

The `hawk2ui-cli` binary drives the authoring workflow:

```bash
cargo run -p hawk2ui-cli -- new            # scaffold a project
cargo run -p hawk2ui-cli -- dev            # watch + hot-reload the native surface
cargo run -p hawk2ui-cli -- build-release  # produce a sealed artifact
cargo run -p hawk2ui-cli -- package-plugin # CLAP / VST3 / AU / standalone
```

See `examples/` for working `manifest.hawk.toml` layouts (`desktop-basic`, `plugin-synth-editor`), and `CLAUDE.md` for the architecture and crate layering.

## Documentation

- Product direction — `docs/specs/0001-product-direction.md`
- Domain spec index — `docs/specs/0002-domain-spec-index.md`
- Rendering architecture — `docs/specs/rendering-architecture.md`
- Crate selection rationale — `docs/technical/crate-selection.md`
- Decisions (repo reset, stable baseline) — `docs/decisions/`
- Domain specs, task lists, and coverage — `specs/`, `tasks/` (`tasks/COVERAGE.md`)
- User-facing manual — `manual/`

## Assets

The Hawk2UI logo assets in `assets/` were carried forward from the archived `hawk2ui-prototypes` repo and remain part of the production project identity.

## License

Dual-licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option — build, ship, and sell desktop apps, plugin editors, plugins, hosts,
and end-user software with no further permission or fee.

One narrow exception ([ADDITIONAL_TERMS.md](ADDITIONAL_TERMS.md)): redistributing
Hawk2UI *itself as a commercial framework* to third-party developers requires a
Framework License. Free / open-source frameworks are exempt, and ambiguities
resolve in favor of the standard grant — no other use is affected.

Unless you state otherwise, any contribution you submit for inclusion is
dual-licensed as above.

## Contact

Maintained by Shawn ([@entrepeneur4lyf](https://github.com/entrepeneur4lyf)).

- **Framework licensing & inquiries** — shawn@hawk2ui.com
- **GitHub** — [@entrepeneur4lyf](https://github.com/entrepeneur4lyf)
- **X** — [@entrepeneur4lyf](https://x.com/entrepeneur4lyf)
