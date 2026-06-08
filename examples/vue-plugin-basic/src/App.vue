<script setup lang="ts">
import { onMounted, ref } from "vue";
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

const gain = ref(0);
const meter = ref("meter:pending");
const transport = ref("transport:pending");
const state = ref("state:pending");
const preset = ref("preset:pending");

onMounted(async () => {
  gain.value = await readParameter("gain");
  const meterFrame = await subscribeMeters({ source: "master" });
  meter.value = `meter:${meterFrame.source}=${meterFrame.values.join(",")}`;
  const snapshot = await getTransport();
  transport.value = `transport:${snapshot.playing}:${snapshot.sampleRate}`;
  const beforeState = await loadState();
  await saveState(JSON.stringify({ preset: "Wide", version: 2 }));
  state.value = `state:${beforeState}->${await loadState()}`;
  const beforePreset = await loadPreset("init");
  await savePreset("init", JSON.stringify({ name: "Wide", version: 2 }));
  preset.value = `preset:${beforePreset}->${await loadPreset("init")}`;
  await sendControl({ type: "ui-ready", source: "vue-plugin-basic" });
});

async function boost() {
  await beginAutomationGesture("gain");
  await writeParameter("gain", 0.75);
  await endAutomationGesture("gain");
  gain.value = await readParameter("gain");
  await sendControl({ type: "automation", parameter: "gain", value: gain.value });
}
</script>

<template>
  <hawk-view id="vue-plugin-root" role="group" aria-label="Vue plugin editor">
    <hawk-text id="gain" role="status" label="Gain">{{ gain.toFixed(2) }}</hawk-text>
    <hawk-text id="meter">{{ meter }}</hawk-text>
    <hawk-text id="transport">{{ transport }}</hawk-text>
    <hawk-text id="state">{{ state }}</hawk-text>
    <hawk-text id="preset">{{ preset }}</hawk-text>
    <hawk-button id="boost" @pointer-press="boost">Boost</hawk-button>
  </hawk-view>
</template>
