export function mount(host) {
  host.on("click", () => host.setState({ ready: true }));
  return {
    id: "desktop-basic-root",
    role: "application",
    text: "Hello Hawk2UI",
  };
}
