# Hawk2UI Project Manifest

`hawk.json` is the canonical project manifest for Hawk2UI applications. It is the project contract in the same sense that `package.json` is the contract for a JavaScript package and `Cargo.toml` is the contract for a Rust crate: every build, launch, package, plugin editor, asset pipeline, permission decision, and release artifact starts from this file.

The legacy `manifest.hawk.toml` format remains a migration input. It is not a parallel long-term format. New project documentation, schemas, examples, and CLI behavior target `hawk.json`.

## Design Rules

- `hawk.json` is the only long-term source manifest format.
- JSON Schema is the public validation contract for tools and editors.
- All relative paths are resolved from the directory containing `hawk.json`.
- Absolute paths, parent-directory escapes, and undeclared asset/script/style inputs are rejected.
- Desktop apps and plugin editors are targets of one Hawk app model, not separate project types.
- Framework selection is an authoring/build input. React release builds consume a sealed JavaScript module graph produced from the selected package-manager build output.
- Release builds must be deterministic: the same manifest, sources, lock state, and signing inputs produce the same artifact hashes.

## Minimal Desktop App

```json
{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {
    "id": "com.example.desktop",
    "name": "Example Desktop",
    "version": "0.1.0",
    "bundleId": "com.example.desktop"
  },
  "app": {
    "entry": "src/App.tsx",
    "framework": "react"
  },
  "targets": {
    "desktop": [
      {
        "name": "main",
        "platforms": ["windows", "macos", "linux-wayland", "linux-x11"],
        "window": {
          "title": "Example Desktop",
          "width": 1280,
          "height": 800,
          "minWidth": 640,
          "minHeight": 400,
          "resizable": true,
          "presentationBackend": "gpu-preferred"
        }
      }
    ]
  }
}
```

## Desktop And Plugin In One Project

```json
{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {
    "id": "com.example.synth",
    "name": "Example Synth",
    "version": "0.1.0",
    "bundleId": "com.example.synth"
  },
  "app": {
    "entry": "src/App.tsx",
    "framework": "react",
    "style": "src/styles.css"
  },
  "targets": {
    "desktop": [
      {
        "name": "standalone",
          "platforms": ["windows", "macos", "linux-wayland", "linux-x11"],
        "window": {
          "title": "Example Synth",
          "width": 1180,
          "height": 720,
          "resizable": true,
          "presentationBackend": "gpu-preferred"
        }
      }
    ],
    "plugin": [
      {
        "name": "editor",
        "formats": ["clap", "vst3", "au"],
        "editor": {
          "width": 960,
          "height": 540,
          "resizable": true
        }
      }
    ]
  },
  "plugin": {
    "id": "com.example.synth",
    "name": "Example Synth",
    "vendor": "Example Audio",
    "parameters": [
      {
        "id": "gain",
        "paramId": 1,
        "name": "Gain",
        "kind": "float",
        "min": -60,
        "max": 12,
        "default": 0,
        "unit": "dB"
      },
      {
        "id": "mode",
        "paramId": 2,
        "name": "Mode",
        "kind": "enum",
        "default": 0,
        "variants": [
          { "id": "clean", "name": "Clean" },
          { "id": "drive", "name": "Drive" }
        ]
      }
    ],
    "meters": [
      { "id": "output.level", "name": "Output Level" }
    ],
    "state": {
      "format": "json-v1"
    }
  },
  "assets": {
    "include": ["assets/**"]
  },
  "permissions": {
    "network": false,
    "filesystem": {
      "read": [],
      "write": []
    },
    "clipboard": "none",
    "secrets": []
  }
}
```

## Top-Level Fields

| Field | Required | Purpose |
|---|---:|---|
| `$schema` | Recommended | Editor/tooling URL for the public JSON Schema. |
| `schemaVersion` | Yes | Integer manifest schema version. The first canonical JSON schema is `1`. |
| `package` | Yes | Stable product identity, display name, version, and bundle/package identifiers. |
| `app` | Yes | `app` declares the authoring entrypoint and framework used to produce package-manager build output. |
| `targets` | Yes | Desktop and/or plugin targets to build from the app model. |
| `plugin` | Required for plugin targets | Plugin identity, parameters, meters, presets, and state contract. |
| `assets` | Optional | Asset include rules and explicit package inputs. |
| `permissions` | Optional | Host service permissions granted to app code. Omitted means deny-by-default. |
| `build` | Optional | Package-manager-produced JavaScript output path, package-manager selection, and lockfile detection. |

Unknown top-level fields are rejected. Unknown nested fields are rejected unless the schema explicitly reserves an extension object.

## `package`

`package` identifies the product across all targets.

```json
{
  "id": "com.example.product",
  "name": "Example Product",
  "version": "1.0.0",
  "bundleId": "com.example.product"
}
```

- `id` is the stable Hawk package id. It must use lowercase ASCII letters, digits, `.`, `_`, or `-`.
- `name` is the human-readable product name.
- `version` is the product version string. SemVer is recommended.
- `bundleId` is the native bundle/package identifier used by OS and plugin packaging.

## `app`

`app` declares the source entry used to build the UI package output.

```json
{
  "entry": "src/App.tsx",
  "framework": "react",
  "style": "src/styles.css",
  "script": "src/runtime.ts"
}
```

Supported `framework` values:

- `native`
- `react`
- `solid`
- `svelte`
- `vue`

`entry` is required. `style` and `script` are optional. React release builds consume a sealed JavaScript module graph produced from the selected package-manager build output. Vue, Solid, and Svelte manifest values remain incubating until their runtime renderer adapters have equivalent release evidence.

## `build`

React release builds read the package-manager-produced JavaScript output declared by `build`.

```json
{
  "output": "dist/main.js",
  "packageManager": "bun"
}
```

`output` is the package-manager-produced JavaScript bundle path sealed into the release artifact for React builds.

`packageManager` accepts `bun`, `npm`, `pnpm`, or `yarn` and selects the lockfile when more than one supported lockfile is present. If `packageManager` is omitted, release builds detect `bun.lock`, `package-lock.json`, `pnpm-lock.yaml`, or `yarn.lock`.

## `targets`

`targets` is an object of target arrays. This shape supports multiple desktop windows/packages, multiple plugin package variants, and future target classes without overloading one flat list.

### Desktop Target

```json
{
  "name": "main",
  "platforms": ["windows", "macos", "linux-wayland", "linux-x11"],
  "window": {
    "title": "Example",
    "width": 1280,
    "height": 800,
    "minWidth": 640,
    "minHeight": 400,
    "resizable": true,
    "presentationBackend": "gpu-preferred"
  }
}
```

Desktop target `platforms` must include Windows, macOS, Linux Wayland, and Linux X11 before a production release claim.

`presentationBackend` values:

- `software`
- `gpu-preferred`
- `gpu-required`

`gpu-required` fails hard when the platform cannot create a GPU surface. `gpu-preferred` falls back to software only when policy allows fallback and diagnostics record the fallback.

### Plugin Target

```json
{
  "name": "editor",
  "formats": ["clap", "vst3", "au"],
  "editor": {
    "width": 960,
    "height": 540,
    "resizable": true
  }
}
```

Supported `formats`: `clap`, `vst3`, and `au`.

Plugin targets require the top-level `plugin` object.

## `plugin`

`plugin` defines host-visible plugin metadata and the editor/control contract.

Parameters use plain denormalized values. `paramId` is optional during authoring but must be pinned before release packaging so host automation, presets, and saved state remain stable.

Parameter kinds:

- `float`
- `int`
- `bool`
- `enum`

Supported units:

- `dB`
- `Hz`
- `ms`
- `s`
- `%`
- `st`
- `pan`
- `""` for unitless parameters

Validation rejects duplicate parameter ids, duplicate pinned `paramId` values, reserved meter id ranges, enum variant collisions, invalid defaults, and unsupported units.

## `assets`

`assets.include` declares package-visible asset globs.

```json
{
  "include": ["assets/**", "fonts/**"]
}
```

Asset paths are workspace-relative. Absolute paths and parent-directory escapes are rejected. Build outputs include source hashes and packaged paths in the sealed artifact.

## `permissions`

Permissions are deny-by-default.

```json
{
  "network": false,
  "filesystem": {
    "read": ["assets/**"],
    "write": []
  },
  "clipboard": "text",
  "secrets": ["license-key"]
}
```

`network` is either `false` or an allow-list object in future schema revisions. `filesystem.read` and `filesystem.write` are scoped path allow lists. `clipboard` is `none` or `text`. `secrets` names declared secret keys that host policy may provide at runtime.

## `build`

`build` configures deterministic output and release policy.

```json
{
  "outDir": "target/hawk2ui",
  "profiles": {
    "dev": {
      "signing": "unsigned-development"
    },
    "release": {
      "signing": "required"
    }
  }
}
```

Release builds require signature metadata and trust verification. Development builds may produce unsigned local artifacts with explicit `unsigned-development` status.

## Validation Rules

The manifest validator must reject:

- Missing `schemaVersion`, `package`, `app`, or `targets`.
- Unknown fields.
- Empty ids, names, versions, target names, or entry paths.
- Unsupported framework names, target keys, target formats, or presentation backends.
- Plugin targets without `plugin` metadata.
- Parameter/meter id collisions.
- Unsafe paths, absolute paths, parent-directory escapes, and undeclared inputs.
- Permissions that request unsupported host capabilities.
- Release profiles without required signing policy.

## CLI Resolution

All project commands resolve `hawk.json` from the current project directory. Legacy `manifest.hawk.toml` is used only when `hawk.json` is absent:

```bash
hawk2ui validate
hawk2ui build-dev
hawk2ui build-release
hawk2ui run-desktop
hawk2ui package-plugin
hawk2ui verify-artifact
hawk2ui diagnostics
```

Convert a legacy project explicitly:

```bash
hawk2ui migrate-manifest
hawk2ui migrate-manifest --force
```

## TOML Migration

`manifest.hawk.toml` maps to `hawk.json` as follows:

| Legacy TOML | Canonical JSON |
|---|---|
| `[identity]` | `package.id`, `package.name`, `package.version` |
| `[package].bundle_id` | `package.bundleId` |
| `[source]` | `app` |
| `[capabilities].keys` | `permissions` or compatibility capability metadata |
| `[[targets]] kind = "desktop"` | `targets.desktop[]` |
| `[[targets]] kind = "plugin"` | `targets.plugin[]` |
| `[plugin]` | `plugin.id`, `plugin.name` |
| `[editor]` | `targets.plugin[].editor` |
| `[[parameters]]` | `plugin.parameters[]` |
| `[[meters]]` | `plugin.meters[]` |
| `[[assets]]` | `assets` declarations |
| `[[presets]]` | `plugin.presets[]` |

The migration command reads legacy TOML, emits `hawk.json`, preserves stable ids and pinned `param_id` values as `paramId`, and refuses to overwrite an existing `hawk.json` unless the user passes `--force`.
