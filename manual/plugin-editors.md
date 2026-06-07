# Hawk2UI Plugin Editors

Plugin editors are Hawk2UI projects with a manifest target whose `kind` is `plugin`. They declare plugin identity, editor dimensions, host-visible parameters, state records, presets, and realtime visual data contracts through the public plugin API surface.

## Fixture-Backed Plugin Manifests

The implemented plugin fixtures are:

- `examples/plugin-basic/hawk.json`: target `vst3-clap`, parameters `gain` and `mix`, editor size `960 x 540`.
- `examples/plugin-synth-editor/hawk.json`: target `clap-vst3`, parameters `osc.mix` and `filter.cutoff`, editor size `960 x 540`.
- `examples/plugin-meter-analyzer/hawk.json`: target `clap-vst3`, capability `realtime-visuals`, editor size `960 x 540`.

A plugin manifest can include:

```toml
[plugin]
id = "com.hawk2ui.examples.plugin-basic"
name = "Hawk2UI Plugin Basic"

[editor]
width = 960
height = 540

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5
```

## Host Formats

The host compatibility matrix covers these formats:

- `clap`
- `vst3`
- `au`
- `standalone`

For each format, the matrix covers host attachment, resize, DPI, keyboard focus, accessibility, state, automation, and realtime visual data.

## Plugin API Records

The public plugin API records are:

- `ParameterId`: stable parameter identifier.
- `AutomationGesture`: host automation gesture record.
- `PluginParameterContract`: parameter metadata and default-value contract.
- `PluginEditorKind`: editor attachment kind.
- `PluginEditorContract`: editor metadata, dimensions, and kind.
- `PluginStateFormat`: plugin state serialization format.
- `PluginStateEntry`: one serialized state entry.
- `PluginStateContract`: complete plugin state contract.
- `PluginPresetContract`: preset metadata and state reference.
- `RealtimeDataKind`: realtime data payload category.
- `RealtimeDataDirection`: realtime data flow direction.
- `RealtimeDataContract`: realtime visual data contract.

## Plugin Workflow

Use this command catalog flow for plugin authoring and packaging:

```bash
hawk2ui validate
hawk2ui package-plugin
hawk2ui verify-artifact
hawk2ui diagnostics
```

The command catalog reserves `hawk2ui package-plugin` for release-backed CLAP and VST3 targets.
The command compiles the generated `cdylib` crates and installs host-loadable shared libraries into the package before final hash verification.
