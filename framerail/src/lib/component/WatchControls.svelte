<script lang="ts">
  import type { WatchSubscription } from "$lib/server/deepwell/watching"

  let {
    watching,
    message
  }: {
    watching: {
      siteId: number
      categoryId: number
      pageId: number
      subscriptions: WatchSubscription[]
    } | null
    message?: string
  } = $props()

  const controls = $derived(
    watching
      ? [
          { scope: "site", targetId: watching.siteId, label: "site" },
          { scope: "category", targetId: watching.categoryId, label: "category" },
          { scope: "page", targetId: watching.pageId, label: "page" }
        ]
      : []
  )
</script>

{#if watching}
  <div class="watch-controls" aria-label="Watching">
    {#if message}<p role="status">{message}</p>{/if}
    {#each controls as control (`${control.scope}:${control.targetId}`)}
      {@const subscribed = watching.subscriptions.some(
        (subscription) =>
          subscription.scope === control.scope &&
          subscription.target_id === control.targetId
      )}
      <form action="?/watching" method="POST">
        <input name="scope" type="hidden" value={control.scope} />
        <input name="target_id" type="hidden" value={control.targetId} />
        <input name="watching" type="hidden" value={String(!subscribed)} />
        <button class="btn btn-default" type="submit">
          {subscribed ? "Unwatch" : "Watch"} this {control.label}
        </button>
      </form>
    {/each}
  </div>
{/if}

<style>
  .watch-controls {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5em;
    margin: 0.5em 0;
  }
</style>
