# Hawk2UI Examples

The examples below are repository fixtures and are used as source-truth coverage for the manuals.

## Desktop

- `examples/desktop-basic/hawk.json`
- `examples/desktop-dashboard/hawk.json`

## Plugins

- `examples/plugin-basic/hawk.json`
- `examples/plugin-synth-editor/hawk.json`
- `examples/plugin-meter-analyzer/hawk.json`

## React And Vue Sealed Runtime

- `examples/react-desktop-basic/hawk.json`
- `examples/react-plugin-basic/hawk.json`
- `examples/vue-desktop-basic/hawk.json`
- `examples/vue-plugin-basic/hawk.json`

These fixtures declare `build.output` and lockfile metadata so smoke tests execute the same sealed JS module graph that packaging records in the artifact. The React and Vue desktop smokes record visible network-update evidence plus storage and file-operation evidence from the sealed bundle; the React and Vue plugin smokes record parameter, state/preset, host transport, realtime meter, DSP control, and realtime-denial evidence.

## Style And Security

- `examples/style-gallery/hawk.json`
- `examples/security-denials/hawk.json`

## Incubating Framework Compiler Fixtures

These fixtures exercise legacy and incubating compiler/reference paths. They are not React or Vue production release evidence; React and Vue production examples are the sealed Deno runtime fixtures above.

- `examples/frameworks/svelte-basic/hawk.json`
- `examples/frameworks/vue-basic/hawk.json`
- `examples/frameworks/solid-basic/hawk.json`
- `examples/frameworks/native-basic/hawk.json`

Use these fixtures as the first reference when checking what the current manifest parser and packaging tests support.
