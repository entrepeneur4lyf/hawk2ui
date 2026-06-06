export function App() {
  const items = [{ id: 'title' }, { id: 'cta' }];
  return <hawk-view id="root" ref="root_ref" className="surface.card" data-asset="assets/logo.svg">
    {items.map((item) => <hawk-text id={item.id} key={item.id}>{item.id}</hawk-text>)}
  </hawk-view>;
}
