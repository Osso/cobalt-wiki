<script lang="ts">
  import { goto } from "$app/navigation"
  import { errorPopupState, pageLayoutState } from "$lib/stores.svelte"
  import { toast } from "$lib/component/scripts/toasts"
  import { Layout, ToastType } from "$lib/types"
  import { resolve } from "$app/paths"
  import { superForm } from "sveltekit-superforms"
  import { untrack } from "svelte"

  import DataFormFields from "$lib/component/DataFormFields.svelte"
  import EditorPreview from "$lib/component/EditorPreview.svelte"
  import EditorDraft from "$lib/component/EditorDraft.svelte"
  import type { PageDraft, PageDraftRequest } from "$lib/server/deepwell/page-draft"
  import WikitextToolbar from "$lib/component/WikitextToolbar.svelte"
  import { createDraft, changedFields, mergeDraftSource } from "$lib/form-editor"
  import type { PageForm } from "$lib/form-editor"

  import type { PageProps } from "./$types"

  let { data, params }: PageProps = $props()

  const sourceForm = untrack(() => data.form)
  let draft = $state(sourceForm ? createDraft(sourceForm) : {})
  let sourceTextarea = $state<HTMLTextAreaElement>()
  let draftControls = $state<{
    cancel: (proceed: () => void) => void
    canPublish: () => boolean
  }>()
  let restoredForm = $state<{ source: string; form: PageForm }>()

  function editorContent() {
    if (!sourceForm) return { wikitext: $form.wikitext ?? "", formUpdates: undefined }
    if (restoredForm) {
      return {
        wikitext: mergeDraftSource(
          restoredForm.source,
          restoredForm.form.values,
          changedFields(restoredForm.form, draft)
        ),
        formUpdates: undefined
      }
    }
    return { wikitext: undefined, formUpdates: changedFields(sourceForm, draft) }
  }

  function draftPayload(): PageDraftRequest {
    const content = editorContent()
    const metadata = {
      title: $form.title ?? "",
      last_revision_id: data.page_revision?.revision_id
    }
    return content.wikitext === undefined
      ? { ...metadata, form_updates: content.formUpdates }
      : { ...metadata, wikitext: content.wikitext }
  }

  function restoreDraft(saved: PageDraft) {
    if (sourceForm) {
      if (!saved.form_values) throw new Error("Saved draft has no form values")
      const form = { ...sourceForm, values: saved.form_values }
      restoredForm = { source: saved.wikitext, form }
      draft = createDraft(form)
    }
    $form.title = saved.title
    $form.wikitext = saved.wikitext
  }

  function requestCancel() {
    draftControls?.cancel(cancelEdit)
  }

  function cancelEdit() {
    const options: string[] = Object.entries({
      norender: data.options.no_render,
      noredirect: data.options.no_redirect
    })
      .filter(([, enabled]) => enabled)
      .map(([key]) => `/${key}`)

    goto(resolve(`/${params.slug}${options.join("")}`, {}), {
      noScroll: true
    })
  }

  const { form, enhance } = superForm(
    untrack(() => data.forms.pageEditForm),
    {
      dataType: "json",
      onSubmit: async ({ jsonData, cancel }) => {
        if (!draftControls?.canPublish()) {
          cancel()
          return
        }
        const submitForm = {
          ...$form,
          ...editorContent(),
          siteId: data.site.site_id,
          pageId: data.page?.page_id,
          lastRevisionId: data.page_revision?.revision_id
        }
        jsonData(submitForm)
      },
      onResult: async ({ result, cancel }) => {
        if (result.type === "success" && result.data) {
          cancel()
          toast(ToastType.Success, data.internationalization!["wiki-page-edit.toast"]!)
          goto(resolve(`/${params.slug}`, {}), {
            noScroll: true
          })
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

  $form.title = untrack(() => data.page_revision?.title ?? "")
  $form.altTitle = untrack(() => data.page_revision?.alt_title ?? "")
  $form.wikitext = untrack(() => data.wikitext)
  $form.tags = untrack(() => data.page_revision?.tags?.join(" ") ?? "")
  $form.comments = untrack(() => data.page_revision?.comments ?? "")
</script>

{#if pageLayoutState.current === Layout.WIKIDOT}
  <h1 class="page-edit-header">
    {data.internationalization?.["wiki-page-edit"]}
  </h1>
{:else}
  <h2 class="page-edit-header">
    {data.internationalization?.["wiki-page-edit"]}
  </h2>
{/if}

<form id="editor" class="editor" action="?/edit" method="POST" use:enhance>
  <input
    name="title"
    class="editor-title"
    placeholder={data.internationalization?.title}
    type="text"
    bind:value={$form.title}
  />
  <input
    name="altTitle"
    class="editor-alt-title"
    placeholder={data.internationalization?.["alt-title"]}
    type="text"
    bind:value={$form.altTitle}
  />
  {#if sourceForm}
    <DataFormFields form={sourceForm} bind:draft />
  {:else}
    <WikitextToolbar textarea={sourceTextarea} bind:value={$form.wikitext} />
    <textarea
      bind:this={sourceTextarea}
      name="wikitext"
      class="editor-wikitext"
      bind:value={$form.wikitext}></textarea>
  {/if}
  <input
    name="tags"
    class="editor-tags"
    placeholder={data.internationalization?.tags}
    type="text"
    bind:value={$form.tags}
  />
  <textarea
    name="comments"
    class="editor-comments"
    placeholder={data.internationalization?.["wiki-page-revision-comments"]}
    bind:value={$form.comments}></textarea>
  {#if pageLayoutState.current === Layout.WIKIDOT}
    <div class="buttons alignleft">
      <input
        name="cancel"
        class="btn btn-danger"
        onclick={requestCancel}
        type="button"
        value={data.internationalization?.cancel}
      />
      <input
        name="save"
        class="btn btn-primary"
        disabled={!draftControls?.canPublish()}
        type="submit"
        value={data.internationalization?.save}
      />
    </div>
  {:else}
    <div class="action-row editor-actions">
      <button
        class="action-button editor-button button-cancel clickable"
        onclick={requestCancel}
        type="button"
      >
        {data.internationalization?.cancel}
      </button>
      <button
        class="action-button editor-button button-save clickable"
        disabled={!draftControls?.canPublish()}
        type="submit"
      >
        {data.internationalization?.save}
      </button>
    </div>
  {/if}
</form>

<EditorPreview
  getPayload={() => ({
    ...draftPayload(),
    alt_title: $form.altTitle || null,
    tags: ($form.tags ?? "").split(/\s+/).filter(Boolean)
  })}
/>
<EditorDraft
  bind:this={draftControls}
  getPayload={draftPayload}
  loadOnMount
  onRestore={restoreDraft}
/>

<style lang="scss">
  .editor-actions {
    padding: 0 0 2em;
  }

  .editor {
    display: flex;
    flex-direction: column;
    gap: 15px;
    align-items: stretch;
    justify-content: stretch;
    width: 100%;
  }

  .editor-wikitext {
    height: 60vh;
  }
</style>
