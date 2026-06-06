<script>
  let label = "Ready";
  let items = [{ id: "feature-native" }, { id: "feature-fast" }];

  function handlePress() {
    label = "Pressed";
  }

  function handleMount() {
    label = "Mounted";
  }

  function handleDestroy() {
    label = "Unmounted";
  }
</script>

<hawk-view id="root" use:root_ref class="surface.card" data-asset="assets/logo.svg" on:mount={handleMount} on:destroy={handleDestroy}>
  <hawk-text id="title">Svelte Hawk2UI</hawk-text>
  <hawk-button id="cta" on:press={handlePress}>{label}</hawk-button>
  {#each items as item (item.id)}
    <hawk-text id={item.id}>{item.id}</hawk-text>
  {/each}
</hawk-view>
