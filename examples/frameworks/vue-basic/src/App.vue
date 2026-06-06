<script setup>
import { ref } from "vue";

const label = ref("Ready");
const items = [{ id: "feature-native" }, { id: "feature-fast" }];

function handlePress() {
  label.value = "Pressed";
}

function handleMount() {
  label.value = "Mounted";
}

function handleUnmount() {
  label.value = "Unmounted";
}
</script>

<template>
  <hawk-view id="root" ref="root_ref" class="surface.card" data-asset="assets/logo.svg" @mounted="handleMount" @unmounted="handleUnmount">
    <hawk-text id="title">Vue Hawk2UI</hawk-text>
    <hawk-button id="cta" @pointerdown="handlePress">{{ label }}</hawk-button>
    <hawk-text v-for="item in items" :id="item.id" :key="item.id">{{ item.id }}</hawk-text>
  </hawk-view>
</template>
