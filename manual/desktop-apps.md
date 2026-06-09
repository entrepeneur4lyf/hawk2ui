# Hawk2UI Desktop Apps

Desktop apps are projects with at least one `targets.desktop[]` manifest entry. The repository fixtures that exercise this path are `examples/desktop-basic/hawk.json` and `examples/desktop-dashboard/hawk.json`.

## Desktop Manifest

The implemented desktop fixtures use this shape:

```json
{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {
    "id": "com.hawk2ui.examples.desktop-basic",
    "name": "Hawk2UI Desktop Basic",
    "version": "0.1.0",
    "bundleId": "com.hawk2ui.examples.desktop-basic"
  },
  "app": {
    "entry": "src/main.ts",
    "framework": "native"
  },
  "targets": {
    "desktop": [
      {
        "name": "main",
        "platforms": ["windows", "macos", "linux-wayland", "linux-x11"]
      }
    ]
  },
  "permissions": {
    "capabilities": ["native-windowing", "sealed-artifacts"],
    "network": false,
    "filesystem": []
  }
}
```

Required canonical fields for a desktop app are `schemaVersion`, `package`, `app`, and at least one `targets.desktop[]` entry. `permissions.capabilities` is used by packaged applications to declare required host services.

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
- `linux-x11-desktop`: Ubuntu 24.04 LTS or newer, X11/XCB windowing, `desktop-bundle` packaging.

Windows, macOS, Linux Wayland, and Linux X11 are required production release platforms. A public release announcement remains blocked until those targets have native window lifecycle coverage for open, close, resize, DPI, input, software/GPU presentation, packaging/signing/notarization where applicable, and recorded release evidence.

The matrix also includes `windows-plugin`, `macos-plugin`, `linux-wayland-plugin`, and `linux-x11-plugin` for truce-backed plugin host coverage.
