# Hawk2UI Desktop Apps

Desktop apps are projects with at least one manifest target whose `kind` is `desktop`. The repository fixtures that exercise this path are `examples/desktop-basic/hawk.json` and `examples/desktop-dashboard/hawk.json`.

## Desktop Manifest

The implemented desktop fixtures use this shape:

```toml
[identity]
id = "com.hawk2ui.examples.desktop-basic"
name = "Hawk2UI Desktop Basic"
version = "0.1.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"
```

Required sections for a desktop app are `[identity]`, `[source]`, and at least one `[[targets]]` entry with `kind = "desktop"`. `[capabilities]` is used by packaged applications to declare required host services.

## Desktop Workflow

Use the command catalog flow in this order:

```bash
hawk2ui validate
hawk2ui build-dev
hawk2ui run-desktop
```

Before release packaging, use:

```bash
hawk2ui build-release
hawk2ui verify-artifact
hawk2ui diagnostics
```

## Supported Desktop Compatibility Targets

The compatibility matrix currently names these desktop targets:

- `windows-desktop`: Windows 10 22H2 or newer, `hwnd` windowing, `desktop-bundle` packaging.
- `macos-desktop`: macOS 13 or newer, `nswindow` windowing, `app-bundle` packaging.
- `linux-wayland-desktop`: Ubuntu 24.04 LTS or newer, native Wayland windowing, `desktop-bundle` packaging.

Windows and macOS are mandatory production release targets. A public release announcement remains blocked until both targets have native window lifecycle coverage for open, close, resize, DPI, input, software/GPU presentation, packaging/signing/notarization where applicable, and recorded release evidence.

The matrix also includes `linux-x11-plugin` for plugin host coverage, not standalone desktop application launch.
