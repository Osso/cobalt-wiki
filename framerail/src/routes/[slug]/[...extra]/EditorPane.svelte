<script lang="ts">
  import { goto } from "$app/navigation"
  import { errorPopupState, pageLayoutState } from "$lib/stores.svelte"
  import { toast } from "$lib/component/scripts/toasts"
  import { Layout, ToastType } from "$lib/types"
  import { resolve } from "$app/paths"
  import { superForm } from "sveltekit-superforms"
  import { untrack } from "svelte"

  import DataFormFields from "$lib/component/DataFormFields.svelte"
  import EditorActions from "$lib/component/EditorActions.svelte"
  import type { PageDraft, PageDraftRequest } from "$lib/server/deepwell/page-draft"
  import WikitextToolbar from "$lib/component/WikitextToolbar.svelte"
  import { lookupEditorPages, lookupEditorAttachments } from "$lib/editor-lookup"
  import { createDraft, changedFields, mergeDraftSource } from "$lib/form-editor"
  import { preventEditorImplicitSubmit } from "$lib/editor-submit"
  import type { PageForm } from "$lib/form-editor"

  import type { PageProps } from "./$types"

  let { data, params }: PageProps = $props()

  const sourceForm = untrack(() => data.form)
  let draft = $state(sourceForm ? createDraft(sourceForm) : {})
  let sourceTextarea = $state<HTMLTextAreaElement>()
  let actions = $state<EditorActions>()
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
        if (!actions?.canPublish()) {
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

<form
  id="editor"
  class="editor"
  action="?/edit"
  method="POST"
  use:enhance
  use:preventEditorImplicitSubmit
>
  {#if sourceForm}
    <DataFormFields
      form={sourceForm}
      titleLabel={data.internationalization?.title}
      bind:draft
      bind:title={$form.title}
    />
  {:else}
    <input
      name="title"
      class="editor-title"
      placeholder={data.internationalization?.title}
      type="text"
      bind:value={$form.title}
    />
    <WikitextToolbar
      attachmentLookup={data.page
        ? () => lookupEditorAttachments(params.slug)
        : undefined}
      pageLookup={lookupEditorPages}
      textarea={sourceTextarea}
      bind:value={$form.wikitext}
    />
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
  <label class="watcher-suppression">
    <input
      name="doNotNotifyWatchers"
      type="checkbox"
      bind:checked={$form.doNotNotifyWatchers}
    />
    Do Not Notify Watchers
  </label>
</form>

<EditorActions
  bind:this={actions}
  cancelLabel={data.internationalization?.cancel}
  getDraftPayload={draftPayload}
  getPreviewPayload={() => ({
    ...draftPayload(),
    alt_title: $form.altTitle || null,
    tags: ($form.tags ?? "").split(/\s+/).filter(Boolean)
  })}
  onCancel={cancelEdit}
  onRestore={restoreDraft}
  saveLabel={data.internationalization?.save}
/>

<style lang="scss">
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

  .watcher-suppression {
    display: flex;
    gap: 0.5em;
    align-items: center;
  }
</style>
