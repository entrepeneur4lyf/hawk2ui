export function App() {
  const [items] = createSignal([{ id: 'title' }, { id: 'cta' }]);
  return <hawk-view id="root" ref={root_ref} class="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onCleanup={onCleanup}>
    <For each={items()}>{(item) => <hawk-text id={item.id}>{item.id}</hawk-text>}</For>
  </hawk-view>;
}
