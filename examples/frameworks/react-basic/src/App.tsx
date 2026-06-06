import { useState } from "react";

export function App() {
  const [label, setLabel] = useState("Ready");
  const items = [{ id: "feature-native" }, { id: "feature-fast" }];

  function handlePress() {
    setLabel("Pressed");
  }

  function handleMount() {
    setLabel("Mounted");
  }

  function handleUnmount() {
    setLabel("Unmounted");
  }

  return (
    <hawk-view
      id="root"
      ref="root_ref"
      className="surface.card"
      data-asset="assets/logo.svg"
      onMount={handleMount}
      onUnmount={handleUnmount}
    >
      <hawk-text id="title">React Hawk2UI</hawk-text>
      <hawk-button id="cta" onPointerDown={handlePress}>{label}</hawk-button>
      {items.map((item) => <hawk-text id={item.id} key={item.id}>{item.id}</hawk-text>)}
    </hawk-view>
  );
}
