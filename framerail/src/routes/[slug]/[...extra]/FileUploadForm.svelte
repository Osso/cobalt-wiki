<script lang="ts">
  import { onDestroy } from "svelte"
  import { pageLayoutState } from "$lib/stores.svelte"
  import { Layout } from "$lib/types"
  import { uploadFiles, type UploadRow } from "$lib/fileUploadBatch"
  import type { PageProps } from "./$types"

  let {
    data,
    onUploaded,
    onClose
  }: {
    data: PageProps["data"]
    onUploaded: () => Promise<void>
    onClose: () => void
  } = $props()

  let files = $state<File[]>([])
  let name = $state("")
  let comments = $state("")
  let rows = $state<UploadRow[]>([])
  let uploading = $state(false)
  let submitted = $state(false)
  let formError = $state("")
  let active = true
  onDestroy(() => {
    active = false
  })

  let uploaded = $derived(rows.filter((row) => row.state === "uploaded").length)
  let failed = $derived(rows.filter((row) => row.state === "failed").length)

  function selectFiles(event: Event) {
    files = Array.from((event.currentTarget as HTMLInputElement).files ?? [])
    rows = files.map((file) => ({ file, state: "queued" }))
    submitted = false
    formError = ""
    name = ""
  }

  async function submitUpload(event: SubmitEvent) {
    event.preventDefault()
    if (uploading || submitted || files.length === 0) return
    const pageId = data.page?.page_id
    const lastRevisionId = data.page_revision?.revision_id
    if (pageId === undefined || lastRevisionId === undefined) {
      formError = "Cannot upload without a page revision."
      return
    }
    uploading = true
    submitted = true
    formError = ""
    const url = new URL("?/fileUpload", window.location.href).href
    const identity = {
      siteId: data.site.site_id,
      pageId,
      lastRevisionId
    }
    try {
      const result = await uploadFiles({
        files: [...files],
        identity,
        name,
        comments,
        url,
        fetchUpload: (target, init) => fetch(target, init),
        isActive: () => active,
        onStatus: (updated) => {
          rows = updated
        },
        onUploaded
      })
      if (active && files.length === 1 && result[0]?.state === "uploaded") onClose()
    } catch (error) {
      if (active) formError = error instanceof Error ? error.message : String(error)
    } finally {
      uploading = false
    }
  }
</script>

<form
  id="file-upload"
  class="file-upload"
  action="?/fileUpload"
  enctype="multipart/form-data"
  method="POST"
  onsubmit={submitUpload}
>
  <div class="file-form-field">
    <label for="file-upload-files"
      >{data.internationalization?.["wiki-page-file-upload.select"]}</label
    >
    <input
      id="file-upload-files"
      name="file"
      class="file-attribute file"
      type="file"
      multiple
      disabled={uploading}
      onchange={selectFiles}
    />
  </div>
  {#if files.length <= 1}
    <div class="file-form-field">
      <label for="file-upload-name"
        >{data.internationalization?.["wiki-page-file-upload.name"]}</label
      >
      <input
        id="file-upload-name"
        name="name"
        class="file-attribute name"
        placeholder={files[0]?.name}
        type="text"
        bind:value={name}
        disabled={uploading}
      />
    </div>
  {/if}
  <div class="file-form-field">
    <label for="file-upload-comments"
      >{data.internationalization?.["wiki-page-revision-comments"] ?? "Comments"}</label
    >
    <textarea
      id="file-upload-comments"
      name="comments"
      class="file-comments"
      bind:value={comments}
      disabled={uploading}></textarea>
  </div>
  {#if rows.length > 0}
    <p role="status">{uploaded} uploaded, {failed} failed of {rows.length}</p>
    <ul class="upload-results">
      {#each rows as row, index (index)}
        <li data-upload-index={index}>
          {row.file.name}: {row.state === "queued"
            ? "Queued"
            : row.state === "uploading"
              ? "Uploading"
              : row.state === "uploaded"
                ? "Uploaded"
                : "Failed"}
          {#if row.error}<span class="upload-error"> — {row.error}</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
  {#if formError}<p class="upload-error" role="alert">{formError}</p>{/if}
  {#if pageLayoutState.current === Layout.WIKIDOT}
    <div class="buttons">
      <input
        class="btn btn-default"
        onclick={onClose}
        type="button"
        value={submitted && files.length > 1
          ? "Close"
          : data.internationalization?.cancel}
        disabled={uploading}
      />
      <input
        class="btn btn-primary"
        type="submit"
        value={data.internationalization?.upload}
        disabled={uploading || submitted || files.length === 0}
      />
    </div>
  {:else}
    <div class="action-row file-upload-actions">
      <button
        class="action-button file-upload-button button-cancel clickable"
        onclick={onClose}
        type="button"
        disabled={uploading}
      >
        {submitted && files.length > 1 ? "Close" : data.internationalization?.cancel}
      </button>
      <button
        class="action-button file-upload-button button-upload clickable"
        type="submit"
        disabled={uploading || submitted || files.length === 0}
      >
        {data.internationalization?.upload}
      </button>
    </div>
  {/if}
</form>

<style lang="scss">
  .file-upload {
    display: flex;
    flex-direction: column;
    gap: 15px;
    align-items: stretch;
    width: 100%;
  }
  .upload-results {
    margin: 0;
    padding-left: 1.5em;
    overflow-wrap: anywhere;
  }
  .upload-error {
    color: #a00;
  }
</style>
