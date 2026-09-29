<script lang="ts">
  import { deserialize } from "$app/forms"
  import type { PagePreviewRequest } from "$lib/server/deepwell/page-preview"
  import { errorPopupState } from "$lib/stores.svelte"
  import type { Snippet } from "svelte"

  interface Props {
    getPayload: () => PagePreviewRequest
    label?: string
    /**
     * Actions rendered in the same row before and after the Preview
     * button.
     */
    before?: Snippet
    after?: Snippet
  }

  let { getPayload, label = "Preview", before, after }: Props = $props()
  let html = $state<string | null>(null)
  let pending = $state(false)
  let errorMessage = $state("")

  function showPreviewError() {
    errorMessage = "Unable to preview page"
    errorPopupState.current = { state: true, message: errorMessage, data: null }
  }

  async function preview() {
    if (pending) return
    pending = true
    html = null
    errorMessage = ""
    try {
      const body = new FormData()
      body.set("payload", JSON.stringify(getPayload()))
      const response = await fetch("?/preview", {
        method: "POST",
        headers: {
          "accept": "application/json",
          "x-sveltekit-action": "true"
        },
        body
      })
      const result = deserialize(await response.text())
      if (result.type === "success" && typeof result.data?.html === "string") {
        html = result.data.html
      } else {
        showPreviewError()
      }
    } catch {
      showPreviewError()
    } finally {
      pending = false
    }
  }
</script>

<div class="editor-action-bar">
  {@render before?.()}
  <button
    id="edit-preview-button"
    class="editor-action"
    disabled={pending}
    onclick={preview}
    type="button"
  >
    {pending ? "Previewing…" : label}
  </button>
  {@render after?.()}
</div>
{#if errorMessage}
  <p role="alert">{errorMessage}</p>
{/if}
<section aria-busy={pending} aria-label="Page preview">
  {#if html !== null}
    {@html html}
  {/if}
</section>
