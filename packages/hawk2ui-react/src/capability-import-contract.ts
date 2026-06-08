import { request } from "hawk:network";
import { getItem, setItem } from "hawk:storage";
import { pickFile } from "hawk:files";
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

async function desktopCapabilityContract() {
  const response = await request("https://example.invalid", { method: "GET" });
  const stored = await getItem("demo", "note");
  await setItem("demo", "note", stored);
  const file = await pickFile();
  return { response, file };
}

function pluginCapabilityContract() {
  beginAutomationGesture("gain");
  writeParameter("gain", 0.5);
  endAutomationGesture("gain");
  const gain = readParameter("gain");
  const state = loadState();
  saveState(state);
  loadPreset("factory.default");
  savePreset("factory.snapshot", state);
  const transport = getTransport();
  const meters = subscribeMeters({ ids: ["main"], intervalMs: 16 });
  sendControl({ type: "gain", value: gain });
  return { gain, transport, meters };
}

void desktopCapabilityContract;
void pluginCapabilityContract;
