<script setup>
const items = [{ id: 'title' }, { id: 'cta' }];
</script>

<template>
  <hawk-view id="root" ref="root_ref" class="surface.card" data-asset="assets/logo.svg">
    <hawk-text v-for="item in items" :id="item.id" :key="item.id">{{ item.id }}</hawk-text>
  </hawk-view>
</template>
