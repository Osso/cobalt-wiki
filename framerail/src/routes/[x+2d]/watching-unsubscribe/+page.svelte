<script lang="ts">
  import type { PageProps } from "./$types"
  let { data }: PageProps = $props()
</script>

<h1>Watcher email</h1>
{#if data.unsubscribed}
  <p>Watcher email is disabled. Your Activity and subscriptions remain available.</p>
{:else}
  <p>
    This unsubscribe link is invalid or expired. Change email preferences in your account
    settings.
  </p>
{/if}
