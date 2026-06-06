import { createSignal, For } from "solid-js";

export function App() {
  const [label, setLabel] = createSignal("Ready");
  const [items] = createSignal([{ id: "feature-native" }, { id: "feature-fast" }]);

  function handlePress() {
    setLabel("Pressed");
  }

  function handleMount() {
    setLabel("Mounted");
  }

  function handleCleanup() {
    setLabel("Unmounted");
  }

  return (
    <hawk-view
      id="root"
      ref="root_ref"
      class="surface.card"
      data-asset="assets/logo.svg"
      onMount={handleMount}
      onCleanup={handleCleanup}
    >
      <hawk-text id="title">Solid Hawk2UI</hawk-text>
      <hawk-button id="cta" onPointerDown={handlePress}>{label()}</hawk-button>
      <For each={items()}>{(item) => <hawk-text id={item.id}>{item.id}</hawk-text>}</For>
    </hawk-view>
  );
}
