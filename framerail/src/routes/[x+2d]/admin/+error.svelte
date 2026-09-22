<script lang="ts">
  import { page } from "$app/state"
</script>

<h1>UNTRANSLATED:Svelte Error</h1>

<p><textarea class="debug">{JSON.stringify(page, null, 2)}</textarea></p>

<!--
Use svelte-switch-case package with {#switch data.view}
as soon as we can figure out prettier support for it.
-->
{#if page.error?.view === "admin_permissions"}
  UNTRANSLATED:Lacks permissions for page
  {#if page.error.html !== undefined}
    {@html page.error.html}
  {/if}
{:else}
  UNTRANSLATED:Fatal error: Unable to display view
{/if}

<style global lang="scss">
  .debug {
    width: 100%;
    height: 60vh;
  }
</style>
