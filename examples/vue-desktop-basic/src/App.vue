<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { request } from "hawk:network";
import { getItem, setItem } from "hawk:storage";
import { pickFile, readText, writeText } from "hawk:files";

const patterns = ["Vue refs", "Computed labels", "Capability-backed state"];
const count = ref(0);
const note = ref("");
const status = ref("loading");
const pickedFile = ref("none");
const countLabel = computed(() => `Count ${count.value}`);

onMounted(async () => {
  const stored = await getItem("vue-desktop-basic", "note");
  note.value = stored;
  const response = await request("https://api.example.test/status");
  status.value = `network:${response.status ?? "ok"}`;
});

async function persistNote(value: string) {
  note.value = value;
  await setItem("vue-desktop-basic", "note", value);
}

async function chooseFile() {
  const file = await pickFile();
  const before = await readText(file);
  await writeText(file, `${before}:saved`);
  pickedFile.value = file;
}
</script>

<template>
  <hawk-view id="vue-desktop-root" role="main" aria-label="Vue desktop demo">
    <hawk-text id="count" role="status" label="Current count">{{ String(count) }}</hawk-text>
    <hawk-text id="count-label">{{ countLabel }}</hawk-text>
    <hawk-text
      v-for="(pattern, index) in patterns"
      :id="`pattern-${index}`"
      :key="pattern"
      role="listitem"
    >
      {{ pattern }}
    </hawk-text>
    <hawk-input
      id="note"
      v-model="note"
      role="textbox"
      aria-label="Demo note"
      @change="persistNote(note)"
    />
    <hawk-button id="increment" @pointer-press="count += 1">Increment</hawk-button>
    <hawk-button id="pick-file" aria-label="Pick a file" @pointer-press="chooseFile">
      Pick file
    </hawk-button>
    <hawk-text v-if="status !== ''" id="status">{{ status }}</hawk-text>
    <hawk-text id="picked-file">{{ pickedFile }}</hawk-text>
  </hawk-view>
</template>
