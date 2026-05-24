<script>
  import logo from '../assets/logo.svg';
  let items = [{ id: 'title' }, { id: 'cta' }];
</script>

<hawk-view id="root" use:ref="root_ref" class="surface.card" data-asset="assets/logo.svg" on:press={handlePress} on:mount={onMount} on:destroy={onDestroy}>
  {#each items as item (item.id)}
    <hawk-text id={item.id}>{item.id}</hawk-text>
  {/each}
</hawk-view>
