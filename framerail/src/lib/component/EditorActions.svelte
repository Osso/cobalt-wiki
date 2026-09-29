<!--
  @component Page editor action bar: Cancel, Preview, Save Draft and Save in one
  row, rendered after the `#editor` form so preview output never nests in it.
-->
<script lang="ts">
  import EditorPreview from "$lib/component/EditorPreview.svelte"
  import EditorDraft from "$lib/component/EditorDraft.svelte"
  import type { PageDraft, PageDraftRequest } from "$lib/server/deepwell/page-draft"
  import type { PagePreviewRequest } from "$lib/server/deepwell/page-preview"

  interface Props {
    cancelLabel?: string
    saveLabel?: string
    onCancel: () => void
    getPreviewPayload: () => PagePreviewRequest
    getDraftPayload: () => PageDraftRequest
    onRestore: (draft: PageDraft) => void
  }

  let {
    cancelLabel,
    saveLabel,
    onCancel,
    getPreviewPayload,
    getDraftPayload,
    onRestore
  }: Props = $props()
  let draft = $state<EditorDraft>()

  export function canPublish() {
    return draft?.canPublish() ?? false
  }
</script>

<EditorPreview getPayload={getPreviewPayload}>
  {#snippet before()}
    <button
      class="editor-action editor-action-cancel"
      onclick={() => draft?.cancel(onCancel)}
      type="button"
    >
      {cancelLabel}
    </button>
    <span class="editor-action-spacer"></span>
  {/snippet}
  {#snippet after()}
    <EditorDraft bind:this={draft} getPayload={getDraftPayload} loadOnMount {onRestore} />
    <button
      class="editor-action editor-action-save"
      disabled={!canPublish()}
      form="editor"
      type="submit"
    >
      {saveLabel}
    </button>
  {/snippet}
</EditorPreview>
