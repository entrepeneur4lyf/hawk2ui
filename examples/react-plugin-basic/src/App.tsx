import React, { useEffect, useState } from "react";
import { createRoot } from "@hawk2ui/react";
import {
  beginAutomationGesture,
  endAutomationGesture,
  getTransport,
  loadPreset,
  loadState,
  readParameter,
  savePreset,
  saveState,
  writeParameter,
} from "hawk:plugin";
import { subscribeMeters } from "hawk:audio";
import { sendControl } from "hawk:dsp";

function App() {
  const [gain, setGain] = useState(0);
  const [meter, setMeter] = useState("meter:pending");
  const [transport, setTransport] = useState("transport:pending");

  useEffect(() => {
    setGain(readParameter("gain"));
    setMeter(JSON.stringify(subscribeMeters({ ids: ["main"], intervalMs: 16 })));
    setTransport(JSON.stringify(getTransport()));
    if (loadState() === "") {
      saveState("gain=0.25");
    }
    loadPreset("factory.default");
    savePreset("factory.snapshot", "gain=0.25");
    sendControl({ type: "ui-ready", source: "react-plugin-basic" });
  }, []);

  function boost() {
    beginAutomationGesture("gain");
    writeParameter("gain", 0.75);
    endAutomationGesture("gain");
    const nextGain = readParameter("gain");
    setGain(nextGain);
    sendControl({ type: "automation", parameter: "gain", value: nextGain });
  }

  return (
    <view id="react-plugin-root" role="group" ariaLabel="React plugin editor">
      <text id="gain" role="status" label="Gain">
        {gain.toFixed(2)}
      </text>
      <text id="meter">{meter}</text>
      <text id="transport">{transport}</text>
      <button id="boost" onPointerPress={boost}>
        Boost
      </button>
    </view>
  );
}

createRoot("editor").render(<App />);
