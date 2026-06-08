import React, { useEffect, useState } from "react";
import { createRoot } from "@hawk2ui/react";
import { request } from "hawk:network";
import { getItem, setItem } from "hawk:storage";
import { pickFile } from "hawk:files";

const items = [
  "Stateful counter",
  "Native list rendering",
  "Capability-backed status",
];

function App() {
  const [count, setCount] = useState(0);
  const [note, setNote] = useState("");
  const [status, setStatus] = useState("loading");
  const [pickedFile, setPickedFile] = useState("none");

  useEffect(() => {
    let cancelled = false;

    async function loadDemoState() {
      const storedNote = await getItem("react-desktop-basic", "note");
      const response = await request("https://example.invalid/hawk2ui/react-demo", {
        method: "GET",
      });
      if (cancelled) return;
      setNote(storedNote);
      setStatus(`network:${response.status ?? "ok"}`);
    }

    loadDemoState().catch((error) => {
      if (!cancelled) setStatus(`error:${String(error)}`);
    });

    return () => {
      cancelled = true;
    };
  }, []);

  async function chooseFile() {
    const file = await pickFile();
    const label = String(file);
    setPickedFile(label);
    await setItem("react-desktop-basic", "lastFile", label);
  }

  return (
    <view id="react-desktop-root" role="main" ariaLabel="React desktop demo">
      <text id="count" role="status" label="Current count">
        {String(count)}
      </text>
      {items.map((item, index) => (
        <text id={`pattern-${index}`} key={item} role="listitem">
          {item}
        </text>
      ))}
      <input
        id="note"
        role="textbox"
        ariaLabel="Demo note"
        value={note}
        onInput={(event) => {
          const value = String((event as { value?: unknown }).value ?? "");
          setNote(value);
          void setItem("react-desktop-basic", "note", value);
        }}
      />
      <button id="increment" onPointerPress={() => setCount(count + 1)}>
        Increment
      </button>
      <button id="pick-file" onPointerPress={chooseFile} ariaLabel="Pick a file">
        Pick file
      </button>
      <text id="status">{status}</text>
      <text id="picked-file">{pickedFile}</text>
    </view>
  );
}

createRoot("main").render(<App />);
