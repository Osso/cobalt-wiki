<script lang="ts">
  import { goto, invalidateAll } from "$app/navigation"
  import { errorPopupState, pageLayoutState } from "$lib/stores.svelte"
  import { DeleteOptions, Layout, ToastType } from "$lib/types"
  import { toast } from "$lib/component/scripts/toasts"
  import { resolve } from "$app/paths"
  import { superForm } from "sveltekit-superforms"
  import { onDestroy, untrack } from "svelte"

  import type { PageProps } from "./$types"

  let { close, data }: PageProps & { close: () => void } = $props()
  let confirmationDialog: HTMLDialogElement | undefined = $state()
  let dismissConfirmation: (() => void) | undefined

  function confirmDeletion(): Promise<boolean> {
    const dialog = confirmationDialog
    if (!dialog) throw new Error("Delete confirmation dialog unavailable")
    if (dialog.open) return Promise.resolve(false)

    dialog.returnValue = ""
    return new Promise((resolve) => {
      const onClose = () => {
        dismissConfirmation = undefined
        resolve(dialog.returnValue === "delete")
      }
      dismissConfirmation = () => {
        dialog.removeEventListener("close", onClose)
        dismissConfirmation = undefined
        resolve(false)
      }
      dialog.addEventListener("close", onClose, { once: true })
      dialog.showModal()
    })
  }

  onDestroy(() => {
    dismissConfirmation?.()
    confirmationDialog?.close()
  })

  const { form, enhance } = superForm(
    untrack(() => data.forms.pageDeleteForm),
    {
      dataType: "json",
      onSubmit: async ({ jsonData, cancel }) => {
        if (
          pageLayoutState.current === Layout.WIKIDOT &&
          $form.option === DeleteOptions.Delete &&
          !(await confirmDeletion())
        ) {
          cancel()
          return
        }
        const submitForm = {
          ...$form,
          siteId: data.site.site_id,
          pageId: data.page?.page_id,
          lastRevisionId: data.page_revision?.revision_id
        }
        jsonData(submitForm)
      },
      onResult: async ({ result, cancel }) => {
        if (result.type === "success" && result.data) {
          if (result.data.option === DeleteOptions.Move) {
            cancel()
            toast(ToastType.Success, data.internationalization!["wiki-page-move.toast"]!)
            goto(resolve(`/${result.data.res.new_slug}`, {}), {
              noScroll: true
            })
            close()
          } else if (result.data.option === DeleteOptions.Delete) {
            toast(
              ToastType.Success,
              data.internationalization!["wiki-page-delete.toast"]!
            )
            close()
            invalidateAll()
          }
        }
        if (result.type === "failure" && result.data) {
          errorPopupState.current = {
            state: true,
            message: result.data.message,
            data: result.data.data
          }
        }
      }
    }
  )

  $form.option = DeleteOptions.Move
  if ($form.option === DeleteOptions.Move) {
    $form.newSlug = `deleted:${untrack(() => data.page?.slug)}`
  }
</script>

{#if pageLayoutState.current === Layout.WIKIDOT}
  <h1 class="page-delete-header">
    {data.internationalization?.["wiki-page-delete"]}
  </h1>
{:else}
  <h2 class="page-delete-header">
    {data.internationalization?.["wiki-page-delete"]}
  </h2>
{/if}

<form id="page-delete" class="page-delete" action="?/delete" method="POST" use:enhance>
  <div>
    <input
      id="page-delete-option-move"
      name="option"
      type="radio"
      value={DeleteOptions.Move}
      bind:group={$form.option}
    />
    <label class="page-delete-option option-move" for="page-delete-option-move">
      {data.internationalization?.["wiki-page-move"]}
    </label>
  </div>
  <div>
    <input
      id="page-delete-option-delete"
      name="option"
      type="radio"
      value={DeleteOptions.Delete}
      bind:group={$form.option}
    />
    <label class="page-delete-option option-delete" for="page-delete-option-delete">
      {data.internationalization?.["wiki-page-delete"]}
    </label>
  </div>

  {#if pageLayoutState.current === Layout.WIKIDOT}
    {#if $form.option === DeleteOptions.Move}
      <input
        name="new-slug"
        class="page-move-new-slug"
        placeholder={data.internationalization?.["wiki-page-move.new-slug"]}
        type="text"
        bind:value={$form.newSlug}
      />
      <textarea
        name="comments"
        class="page-move-comments"
        placeholder={data.internationalization?.["wiki-page-revision-comments"]}
        bind:value={$form.comments}></textarea>
    {/if}
    <div class="buttons">
      <input
        class="btn btn-danger"
        onclick={close}
        type="button"
        value={data.internationalization?.cancel}
      />
      <input
        class="btn btn-primary"
        type="submit"
        value={data.internationalization?.confirm}
      />
    </div>
  {:else}
    {#if $form.option === DeleteOptions.Move}
      <input
        name="new-slug"
        class="page-move-new-slug"
        placeholder={data.internationalization?.["wiki-page-move.new-slug"]}
        type="text"
        bind:value={$form.newSlug}
      />
    {/if}
    <textarea
      name="comments"
      class="page-move-comments"
      placeholder={data.internationalization?.["wiki-page-revision-comments"]}
      bind:value={$form.comments}></textarea>
    <div class="action-row page-delete-actions">
      <button
        class="action-button page-delete-button button-cancel clickable"
        onclick={close}
        type="button"
      >
        {data.internationalization?.cancel}
      </button>
      <button
        class="action-button page-delete-button button-confirm clickable"
        type="submit"
      >
        {data.internationalization?.confirm}
      </button>
    </div>
  {/if}
</form>

<dialog bind:this={confirmationDialog} aria-labelledby="delete-confirmation-title">
  <h2 id="delete-confirmation-title">Delete page?</h2>
  <p>Are you sure you want to completely wipe out this page?</p>
  <form class="confirmation-actions" method="dialog">
    <button type="submit" value="cancel">Cancel</button>
    <button class="btn btn-danger" type="submit" value="delete">Delete page</button>
  </form>
</dialog>

<style lang="scss">
  dialog {
    max-width: min(28rem, calc(100vw - 2rem));
    padding: 1.5rem;
    border: 1px solid currentColor;
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 50%);
  }

  .confirmation-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }

  .page-delete {
    display: flex;
    flex-direction: column;
    gap: 15px;
    align-items: stretch;
    justify-content: stretch;
    width: 100%;
    padding: 0 0 2em;
  }
</style>
