<script lang="ts">
  import { deserialize } from "$app/forms"
  import { goto } from "$app/navigation"
  import { resolve } from "$app/paths"
  import { onDestroy } from "svelte"
  import CharacterRenameConfirm from "./CharacterRenameConfirm.svelte"

  import type {
    CharacterRenamePreview,
    CharacterRenameStatus
  } from "$lib/server/deepwell/character-rename"
  import type { PageProps } from "./$types"

  const POLL_INTERVAL_MS = 1000

  let { data, close }: PageProps & { close: () => void } = $props()

  let newName = $state("")
  let preview = $state<CharacterRenamePreview | null>(null)
  let progress = $state<CharacterRenameStatus | null>(null)
  let elapsedSeconds = $state(0)
  let busy = $state(false)
  let error = $state("")
  let requestId = 0

  // Stops polling only; a started rename keeps running in Deepwell.
  onDestroy(() => requestId++)

  async function post<T extends Record<string, unknown>>(
    action: string,
    fields: Record<string, string>
  ): Promise<T> {
    const body = new FormData()
    for (const [name, value] of Object.entries(fields)) body.set(name, value)
    const response = await fetch(`?/${action}`, { method: "POST", body })
    const result = deserialize<T, { message: string }>(await response.text())
    if (result.type === "success" && result.data) return result.data
    throw new Error(
      result.type === "failure" && result.data
        ? result.data.message
        : "UNTRANSLATED:Character rename request failed"
    )
  }

  /** Runs one request chain; a newer chain or closing the pane silences it. */
  async function run(task: (current: number) => Promise<void>) {
    const current = ++requestId
    busy = true
    error = ""
    try {
      await task(current)
    } catch (cause) {
      if (current === requestId) {
        error = cause instanceof Error ? cause.message : String(cause)
      }
    } finally {
      if (current === requestId) busy = false
    }
  }

  function cancel() {
    requestId++
    close()
  }

  function showPreview(event: SubmitEvent) {
    event.preventDefault()
    return run(async (current) => {
      const result = await post<{ preview: CharacterRenamePreview }>(
        "characterRenamePreview",
        { pageId: String(data.page?.page_id ?? ""), newName }
      )
      if (current === requestId) preview = result.preview
    })
  }

  function confirmRename(confirmed: CharacterRenamePreview) {
    return run(async (current) => {
      const { operationId } = await post<{ operationId: number }>(
        "characterRenameStart",
        {
          pageId: String(data.page?.page_id ?? ""),
          newName: confirmed.new_name,
          previewToken: confirmed.preview_token,
          confirmed: "yes"
        }
      ).catch((cause) => {
        // Nothing started; every refusal needs a fresh preview.
        preview = null
        throw cause
      })
      await followRename(operationId, current)
    })
  }

  async function followRename(operationId: number, current: number) {
    const startedAt = Date.now()
    while (current === requestId) {
      const { progress: next } = await post<{ progress: CharacterRenameStatus }>(
        "characterRenameStatus",
        { operationId: String(operationId) }
      )
      if (current !== requestId) return
      progress = next
      elapsedSeconds = Math.round((Date.now() - startedAt) / 1000)
      if (next.status === "done") {
        await goto(resolve(`/${next.new_slug}`, {}))
        close()
        return
      }
      if (next.status === "failed") {
        throw new Error(next.error ?? "UNTRANSLATED:Character rename failed")
      }
      await new Promise((wake) => setTimeout(wake, POLL_INTERVAL_MS))
    }
  }
</script>

<h1 class="character-rename-header">Rename character</h1>
<p>
  Changes the page URL, tags, writing/arc/chain name lists, relationship boxes, links and
  icon. The display name is not changed.
</p>

{#if progress && preview}
  <p role="status">
    Renaming {preview.old_name} → {preview.new_name}: {progress.done}/{progress.total} pages
    ({elapsedSeconds}s)
  </p>
{:else if preview}
  <CharacterRenameConfirm
    {preview}
    {busy}
    {cancel}
    confirm={() => confirmRename(preview!)}
  />
{:else}
  <form class="character-rename" onsubmit={showPreview}>
    <label>
      New name
      <input
        name="newName"
        class="text"
        disabled={busy}
        required
        type="text"
        bind:value={newName}
      />
    </label>
    <div class="buttons">
      <input class="btn btn-danger" onclick={cancel} type="button" value="Cancel" />
      <input class="btn btn-primary" disabled={busy} type="submit" value="Preview" />
    </div>
  </form>
{/if}

{#if error}
  <p class="error-block" role="alert">{error}</p>
{/if}

<style lang="scss">
  .character-rename {
    display: flex;
    flex-direction: column;
    gap: 15px;
    padding: 0 0 2em;
  }
</style>
