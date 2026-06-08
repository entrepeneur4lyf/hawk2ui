{
  const handlers = new Map();
  let status = "Ready";

  function commit(batch) {
    globalThis.__hawk2uiCommitScene(batch);
  }

  function handlerKey(id, event) {
    return `${id}:${event}`;
  }

  function registerEvent(id, event, handlerId, handler) {
    handlers.set(handlerKey(id, event), handler);
    return { type: "register-event", id, event, handler: handlerId };
  }

  function renderInitialStatus() {
    commit({
      ops: [
        { type: "create-node", id: "root", kind: "view" },
        { type: "create-node", id: "status", kind: "text" },
        { type: "set-prop", id: "status", name: "text", value: { kind: "string", value: status } },
        { type: "append-child", parent: "root", child: "status" },
        { type: "create-node", id: "press", kind: "button" },
        { type: "set-prop", id: "press", name: "text", value: { kind: "string", value: "Press" } },
        registerEvent("press", "pointer.press", "press", press),
        { type: "append-child", parent: "root", child: "press" },
        { type: "commit" },
      ],
    });
  }

  function press() {
    status = "Pressed";
    commit({
      ops: [
        { type: "set-prop", id: "status", name: "text", value: { kind: "string", value: status } },
        { type: "commit" },
      ],
    });
  }

  Object.defineProperty(globalThis, "__hawk2uiDispatchEvent", {
    value(id, event, payload) {
      const handler = handlers.get(handlerKey(id, event));
      if (!handler) {
        throw new Error(`vue.fixture.handler-missing: ${id}:${event}`);
      }
      handler(payload);
    },
    writable: false,
    enumerable: false,
    configurable: false,
  });

  renderInitialStatus();
}
