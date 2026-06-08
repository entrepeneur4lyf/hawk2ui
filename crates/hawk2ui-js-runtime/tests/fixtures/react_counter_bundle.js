{
  const handlers = new Map();
  let count = 0;

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

  function renderInitialCounter() {
    commit({
      ops: [
        { type: "create-node", id: "root", kind: "view" },
        { type: "create-node", id: "count", kind: "text" },
        { type: "set-prop", id: "count", name: "text", value: { kind: "string", value: String(count) } },
        { type: "append-child", parent: "root", child: "count" },
        { type: "create-node", id: "increment", kind: "button" },
        { type: "set-prop", id: "increment", name: "text", value: { kind: "string", value: "Increment" } },
        registerEvent("increment", "pointer.press", "increment", increment),
        { type: "append-child", parent: "root", child: "increment" },
        { type: "commit" },
      ],
    });
  }

  function increment() {
    count += 1;
    commit({
      ops: [
        { type: "set-prop", id: "count", name: "text", value: { kind: "string", value: String(count) } },
        { type: "commit" },
      ],
    });
  }

  Object.defineProperty(globalThis, "__hawk2uiDispatchEvent", {
    value(id, event, payload) {
      const handler = handlers.get(handlerKey(id, event));
      if (!handler) {
        throw new Error(`react.fixture.handler-missing: ${id}:${event}`);
      }
      handler(payload);
    },
    writable: false,
    enumerable: false,
    configurable: false,
  });

  renderInitialCounter();
}
