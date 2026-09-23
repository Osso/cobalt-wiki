<script lang="ts">
  import { deserialize } from "$app/forms"
  import { onMount, tick } from "svelte"
  import type {
    PageDraft,
    PageDraftRequest,
    PageDraftResponse
  } from "$lib/server/deepwell/page-draft"

  interface Props {
    getPayload: () => PageDraftRequest
    onRestore: (draft: PageDraft) => void
    loadOnMount?: boolean
  }

  let { getPayload, onRestore, loadOnMount = false }: Props = $props()
  let draft = $state<PageDraft | null>(null)
  let pending = $state(false)
  let loaded = $state(false)
  let choice = $state<"restore" | "cancel" | null>(null)
  let message = $state("")
  let errorMessage = $state("")
  let pendingCancel: (() => void) | null = null
  let prompt = $state<HTMLDialogElement>()

  async function focusPrompt() {
    await tick()
    prompt?.showModal()
  }

  async function sendAction(
    action: string,
    payload?: PageDraftRequest
  ): Promise<unknown> {
    const body = new FormData()
    if (payload !== undefined) body.set("payload", JSON.stringify(payload))
    const response = await fetch(`?/${action}`, {
      method: "POST",
      headers: { accept: "application/json", "x-sveltekit-action": "true" },
      body
    })
    const result = deserialize(await response.text())
    if (result.type !== "success") throw new Error(`Unable to ${action} draft`)
    return result.data
  }

  function readDraft(value: unknown): PageDraft | null {
    const response = value as PageDraftResponse
    if (!response || !Object.hasOwn(response, "draft")) {
      throw new Error("Invalid draft response")
    }
    return response.draft
  }

  export function canPublish() {
    return loaded && !pending && choice === null
  }

  export async function loadDraft() {
    if (pending) return
    loaded = false
    pending = true
    message = ""
    errorMessage = ""
    try {
      draft = readDraft(await sendAction("draftGet"))
      loaded = true
      choice = draft === null ? null : "restore"
      if (choice) await focusPrompt()
    } catch {
      errorMessage = "Unable to load draft"
    } finally {
      pending = false
    }
  }

  async function saveDraft() {
    if (pending) return
    pending = true
    message = ""
    errorMessage = ""
    try {
      const saved = readDraft(await sendAction("draftSave", getPayload()))
      if (saved === null) throw new Error("Missing saved draft")
      draft = saved
      choice = null
      message = "Draft saved"
    } catch {
      errorMessage = "Unable to save draft"
    } finally {
      pending = false
    }
  }

  async function deleteDraft() {
    if (pending) return
    pending = true
    errorMessage = ""
    try {
      const response = (await sendAction("draftDelete")) as { deleted?: boolean }
      if (response?.deleted !== true) throw new Error("Draft was not deleted")
      draft = null
      choice = null
      message = "Draft deleted"
      const callback = pendingCancel
      pendingCancel = null
      callback?.()
    } catch {
      errorMessage = "Unable to delete draft"
    } finally {
      pending = false
    }
  }

  export function cancel(onCancel: () => void) {
    if (pending) return
    if (draft === null) {
      onCancel()
      return
    }
    pendingCancel = onCancel
    choice = "cancel"
    message = ""
    void focusPrompt()
  }

  function restoreDraft() {
    if (draft === null) return
    try {
      onRestore(draft)
      choice = null
      message = "Draft restored"
    } catch {
      errorMessage = "Unable to restore draft in this editor"
    }
  }

  function leaveDraft() {
    const callback = pendingCancel
    pendingCancel = null
    choice = null
    callback?.()
  }

  onMount(() => {
    if (loadOnMount) void loadDraft()
    else loaded = true
  })
</script>

<button
  id="edit-save-draft-button"
  disabled={!loaded || pending || choice === "restore"}
  onclick={saveDraft}
  type="button"
>
  {pending ? "Working…" : "Save Draft"}
</button>
{#if message}<p role="status">{message}</p>{/if}
{#if errorMessage}<p role="alert">{errorMessage}</p>{/if}
{#if choice === "restore" && draft}
  <dialog bind:this={prompt} aria-label="Saved draft" oncancel={() => (choice = null)}>
    <p>A saved draft exists. Choose which version to edit.</p>
    <button disabled={pending} onclick={() => (choice = null)} type="button">
      Edit Original
    </button>
    <button disabled={pending} onclick={restoreDraft} type="button">Edit Draft</button>
  </dialog>
{:else if choice === "cancel"}
  <dialog
    bind:this={prompt}
    aria-label="Keep saved draft"
    oncancel={() => (choice = null)}
  >
    <p>Delete saved draft or leave it for later?</p>
    <button disabled={pending} onclick={deleteDraft} type="button">Delete Draft</button>
    <button disabled={pending} onclick={leaveDraft} type="button">Leave Draft</button>
    <button disabled={pending} onclick={() => (choice = null)} type="button">
      Continue Editing
    </button>
  </dialog>
{/if}
