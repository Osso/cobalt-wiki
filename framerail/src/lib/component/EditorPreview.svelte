<script lang="ts">
  import { deserialize } from "$app/forms"
  import type { PagePreviewRequest } from "$lib/server/deepwell/page-preview"
  import { errorPopupState } from "$lib/stores.svelte"

  interface Props {
    getPayload: () => PagePreviewRequest
    label?: string
  }

  let { getPayload, label = "Preview" }: Props = $props()
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

<button id="edit-preview-button" disabled={pending} onclick={preview} type="button">
  {pending ? "Previewing…" : label}
</button>
{#if errorMessage}
  <p role="alert">{errorMessage}</p>
{/if}
<section aria-busy={pending} aria-label="Page preview">
  {#if html !== null}
    {@html html}
  {/if}
</section>
