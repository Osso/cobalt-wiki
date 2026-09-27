<script lang="ts">
  import { deserialize } from "$app/forms"
  import { page } from "$app/state"
  import { invalidateAll } from "$app/navigation"
  import { errorPopupState, pageLayoutState } from "$lib/stores.svelte"
  import { Layout, ToastType } from "$lib/types"
  import { toast } from "$lib/component/scripts/toasts"
  import { SvelteMap } from "svelte/reactivity"
  import { fileProxy, superForm } from "sveltekit-superforms"
  import { untrack } from "svelte"
  import FileUploadForm from "./FileUploadForm.svelte"
  import { formatFileSize } from "$lib/fileSize"

  import type { PageProps } from "./$types"
  import type { PageFile, PageFileDelete } from "$lib/server/deepwell/pageFile"
  import type { FileRevisionModel, Optional } from "$lib/types"

  let { data, initialFiles = [] }: PageProps & { initialFiles?: PageFile[] } = $props()

  type FileAction = "upload" | "edit" | "move" | "restore" | "history"
  let activeFileAction = $state<FileAction | null>(null)

  let fileMap = new SvelteMap<number, PageFile>(
    untrack(() => initialFiles.map((file) => [file.file_id, file]))
  )
  let listedFiles = $derived(
    [...fileMap.values()].filter((file) => file.revision_type !== "delete")
  )
  let totalFileSize = $derived(listedFiles.reduce((total, file) => total + file.size, 0))
  let fileEditId = $state<number>(0)
  let fileRevisionMap = new SvelteMap<number, FileRevisionModel>()

  function fileExtension(name: string): string {
    const dot = name.lastIndexOf(".")
    return dot > 0 ? name.slice(dot + 1, dot + 5).toUpperCase() : "FILE"
  }

  function formatListDate(value: string): string {
    return new Date(value).toLocaleDateString(undefined, { dateStyle: "medium" })
  }

  async function getFileList(deleted = false) {
    const res = await fetch("?/fileList", {
      method: "POST",
      body: JSON.stringify({
        siteId: data.site.site_id,
        pageId: data.page?.page_id,
        deleted
      })
    }).then((res) => res.text())

    const result = deserialize<
      { res: PageFile[] },
      { message: string; code: string; data: Record<string, unknown> }
    >(res)

    if (result.type === "failure" && result.data?.message) {
      errorPopupState.current = {
        state: true,
        message: result.data.message,
        data: result.data
      }
    } else if (result.type === "success" && result.data?.res) {
      fileMap.clear()
      result.data.res.forEach((file: PageFile) => {
        fileMap.set(file.file_id, file)
      })
    }
  }

  async function deleteFile(fileId: number, lastRevisionId: number) {
    const res = await fetch("?/fileDelete", {
      method: "POST",
      body: JSON.stringify({
        siteId: data.site.site_id,
        pageId: data.page?.page_id,
        fileId,
        lastRevisionId
      })
    }).then((res) => res.text())

    const result = deserialize<
      { res: PageFileDelete },
      { message: string; code: string; data: Record<string, unknown> }
    >(res)

    if (result.type === "failure" && result.data?.message) {
      errorPopupState.current = {
        state: true,
        message: result.data.message,
        data: result.data
      }
    } else if (result.type === "success" && result.data?.res) {
      toast(ToastType.Success, data.internationalization!["wiki-page-file-delete.toast"]!)
      activeFileAction = null
      await getFileList()
    }
  }

  const {
    form: editForm,
    enhance: editEnhance,
    reset: editReset
  } = superForm(
    untrack(() => data.forms.fileEditForm),
    {
      dataType: "json",
      onSubmit: async ({ jsonData }) => {
        const submitForm = {
          ...$editForm,
          siteId: data.site.site_id,
          pageId: data.page?.page_id,
          fileId: fileEditId,
          lastRevisionId: fileMap.get(fileEditId)?.revision_id
        }
        jsonData(submitForm)
      },
      onResult: async ({ result }) => {
        if (result.type === "success" && result.data) {
          activeFileAction = null
          await getFileList()
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
  const editFile = fileProxy(editForm, "file")

  const {
    form: moveForm,
    enhance: moveEnhance,
    reset: moveReset
  } = superForm(
    untrack(() => data.forms.fileMoveForm),
    {
      dataType: "json",
      onSubmit: async ({ jsonData }) => {
        const submitForm = {
          ...$moveForm,
          siteId: data.site.site_id,
          pageId: data.page?.page_id,
          fileId: fileEditId,
          lastRevisionId: fileMap.get(fileEditId)?.revision_id
        }
        jsonData(submitForm)
      },
      onResult: async ({ result }) => {
        if (result.type === "success" && result.data) {
          activeFileAction = null
          await getFileList()
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

  const {
    form: restoreForm,
    enhance: restoreEnhance,
    reset: restoreReset
  } = superForm(
    untrack(() => data.forms.fileRestoreForm),
    {
      dataType: "json",
      onSubmit: async ({ jsonData }) => {
        const submitForm = {
          ...$restoreForm,
          siteId: data.site.site_id,
          pageId: data.page?.page_id,
          fileId: fileEditId
        }
        jsonData(submitForm)
      },
      onResult: async ({ result }) => {
        if (result.type === "success" && result.data) {
          toast(
            ToastType.Success,
            data.internationalization!["wiki-page-file-restore.toast"]!
          )
          activeFileAction = null
          await getFileList()
          await invalidateAll()
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

  async function handleFileHistory(fileId: number) {
    const res = await fetch("?/fileHistory", {
      method: "POST",
      body: JSON.stringify({
        siteId: data.site.site_id,
        pageId: data.page?.page_id,
        fileId
      })
    }).then((res) => res.text())

    const result = deserialize<
      { res: FileRevisionModel[] },
      { message: string; code: string; data: Record<string, unknown> }
    >(res)

    if (result.type === "failure" && result.data?.message) {
      errorPopupState.current = {
        state: true,
        message: result.data.message,
        data: result.data
      }
    } else if (result.type === "success" && result.data?.res) {
      fileRevisionMap.clear()
      result.data.res.forEach((rev) => {
        fileRevisionMap.set(rev.revision_number, rev)
      })
      activeFileAction = "history"
    }
  }

  async function rollbackFileRevision(revisionNumber: number, comments?: string) {
    const res = await fetch("?/fileRollback", {
      method: "POST",
      body: JSON.stringify({
        siteId: data.site.site_id,
        pageId: data.page?.page_id,
        fileId: fileEditId,
        revisionNumber,
        lastRevisionId: fileMap.get(fileEditId)?.revision_id,
        comments
      })
    }).then((res) => res.text())

    const result = deserialize<
      { res: Optional<PageFile> },
      { message: string; code: string; data: Record<string, unknown> }
    >(res)

    if (result.type === "failure" && result.data?.message) {
      errorPopupState.current = {
        state: true,
        message: result.data.message,
        data: result.data
      }
    } else if (result.type === "success" && result.data?.res) {
      await getFileList()
      activeFileAction = null
      fileRevisionMap.clear()
      await handleFileHistory(fileEditId)
      await invalidateAll()
    }
  }

  $effect(() => {
    getFileList(false)
  })
</script>

{#if pageLayoutState.current === Layout.WIKIDOT}
  <h1 class="page-file-header">
    {data.internationalization?.["wiki-page-file"]}
  </h1>
{:else}
  <h2 class="page-file-header">
    {data.internationalization?.["wiki-page-file"]}
  </h2>
{/if}

<div class="file-panel">
  {#if pageLayoutState.current === Layout.WIKIDOT}
    <div class="buttons">
      {#if activeFileAction !== "upload"}
        <input
          class="btn btn-primary"
          onclick={() => (activeFileAction = "upload")}
          type="button"
          value={data.internationalization?.upload}
        />
      {/if}
      <input
        class="btn btn-default"
        onclick={() => getFileList(true)}
        type="button"
        value={data.internationalization?.restore}
      />
    </div>
  {:else}
    <div class="action-row file-action">
      {#if activeFileAction !== "upload"}
        <button
          class="action-button upload-file clickable"
          onclick={() => (activeFileAction = "upload")}
          type="button"
        >
          {data.internationalization?.upload}
        </button>
      {/if}
      <button
        class="action-button deleted-file clickable"
        onclick={() => getFileList(true)}
        type="button"
      >
        {data.internationalization?.restore}
      </button>
    </div>
  {/if}

  {#if fileMap.size > 0}
    <div class="file-list" class:has-mime={pageLayoutState.current !== Layout.WIKIDOT}>
      <div class="file-list-header">
        <div class="file-attribute name">
          {data.internationalization?.["wiki-page-file.name"]}
        </div>
        <div class="file-attribute created-at">
          {data.internationalization?.["wiki-page-file.created-at"]}
        </div>
        <div class="file-attribute updated-at">
          {data.internationalization?.["wiki-page-file.updated-at"]}
        </div>
        {#if pageLayoutState.current !== Layout.WIKIDOT}
          <div class="file-attribute mime">
            {data.internationalization?.["wiki-page-file.mime"]}
          </div>
        {/if}
        <div class="file-attribute size">
          {data.internationalization?.["wiki-page-file.size"]}
        </div>
        <div class="file-attribute action"></div>
      </div>
      {#each [...fileMap].sort((a, b) => b[0] - a[0]) as [id, file] (id)}
        {@const fileUrl = new URL(
          `/-/file/${data.page?.slug}/${encodeURIComponent(file.name)}`,
          page.url
        ).href}
        <div
          class="file-row"
          class:is-deleted={file.revision_type === "delete"}
          data-id={id}
        >
          <div class="file-attribute name">
            <span class="file-ext" aria-hidden="true">{fileExtension(file.name)}</span>
            <a href={fileUrl} rel="external">
              {file.name}
            </a>
          </div>
          <div class="file-attribute created-at">
            <time
              datetime={file.file_created_at}
              title={new Date(file.file_created_at).toLocaleString()}
              >{formatListDate(file.file_created_at)}</time
            >
          </div>
          <div class="file-attribute updated-at">
            {#if file.file_updated_at}
              <time
                datetime={file.file_updated_at}
                title={new Date(file.file_updated_at).toLocaleString()}
                >{formatListDate(file.file_updated_at)}</time
              >
            {:else}
              <span class="file-empty" aria-label="Never updated">—</span>
            {/if}
          </div>
          {#if pageLayoutState.current !== Layout.WIKIDOT}
            <div class="file-attribute mime">
              {file.mime}
            </div>
          {/if}
          <div
            class="file-attribute size"
            title="{file.size.toLocaleString('en-US')} Bytes"
          >
            {formatFileSize(file.size)}
          </div>
          <div class="file-attribute action">
            {#if pageLayoutState.current === Layout.WIKIDOT}
              {#if file.revision_type === "delete"}
                <!-- svelte-ignore a11y_invalid_attribute -->
                <a
                  class="btn btn-primary btn-sm btn-small"
                  href="javascript:;"
                  onclick={() => {
                    fileEditId = file.file_id
                    activeFileAction = "restore"
                  }}
                >
                  {data.internationalization?.restore}
                </a>
              {:else}
                <details class="file-information">
                  <summary>info</summary>
                  <div class="file-information-content">
                    <h2>File Information</h2>
                    <dl>
                      <dt>File name</dt>
                      <dd>{file.name}</dd>
                      <dt>Full file URL</dt>
                      <dd>
                        <a href={fileUrl} rel="external">{fileUrl}</a>
                      </dd>
                      <dt>File size</dt>
                      <dd>{file.size.toLocaleString("en-US")} Bytes</dd>
                      <dt>MIME type</dt>
                      <dd>{file.mime}</dd>
                      <dt>Local file created</dt>
                      <dd>{new Date(file.file_created_at).toLocaleString()}</dd>
                      {#if file.revision_comments && !file.hidden_fields.includes("comments")}
                        <dt>Revision comment</dt>
                        <dd>{file.revision_comments}</dd>
                      {/if}
                    </dl>
                  </div>
                </details>
                <!-- svelte-ignore a11y_invalid_attribute -->
                <a
                  class="btn btn-primary btn-sm btn-small"
                  href="javascript:;"
                  onclick={() => {
                    activeFileAction = "history"
                    handleFileHistory(file.file_id)
                  }}
                >
                  {data.internationalization?.history}
                </a>
                <!-- svelte-ignore a11y_invalid_attribute -->
                <a
                  class="btn btn-primary btn-sm btn-small"
                  href="javascript:;"
                  onclick={() => {
                    fileEditId = file.file_id
                    activeFileAction = "move"
                  }}
                >
                  {data.internationalization?.move}
                </a>
                <!-- svelte-ignore a11y_invalid_attribute -->
                <a
                  class="btn btn-primary btn-sm btn-small"
                  href="javascript:;"
                  onclick={() => {
                    fileEditId = file.file_id
                    activeFileAction = "edit"
                  }}
                >
                  {data.internationalization?.edit}
                </a>
                <!-- svelte-ignore a11y_invalid_attribute -->
                <a
                  class="btn btn-primary btn-sm btn-small is-danger"
                  href="javascript:;"
                  onclick={() => {
                    deleteFile(file.file_id, file.revision_id)
                  }}
                >
                  {data.internationalization?.delete}
                </a>
              {/if}
            {:else if file.revision_type === "delete"}
              <button
                class="action-button restore-file clickable"
                onclick={() => {
                  fileEditId = file.file_id
                  activeFileAction = "restore"
                }}
                type="button"
              >
                {data.internationalization?.restore}
              </button>
            {:else}
              <button
                class="action-button file-history clickable"
                onclick={() => {
                  activeFileAction = "history"
                  handleFileHistory(file.file_id)
                }}
                type="button"
              >
                {data.internationalization?.history}
              </button>
              <button
                class="action-button move-file clickable"
                onclick={() => {
                  fileEditId = file.file_id
                  activeFileAction = "move"
                }}
                type="button"
              >
                {data.internationalization?.move}
              </button>
              <button
                class="action-button edit-file clickable"
                onclick={() => {
                  fileEditId = file.file_id
                  activeFileAction = "edit"
                }}
                type="button"
              >
                {data.internationalization?.edit}
              </button>
              <button
                class="action-button delete-file clickable"
                onclick={() => deleteFile(file.file_id, file.revision_id)}
                type="button"
              >
                {data.internationalization?.delete}
              </button>
            {/if}
          </div>
        </div>
      {/each}
      {#if pageLayoutState.current === Layout.WIKIDOT && listedFiles.length > 0}
        <div class="file-list-footer">
          <span class="file-count">
            {listedFiles.length}
            {listedFiles.length === 1 ? "file" : "files"}
          </span>
          <p>Total files size: {totalFileSize.toLocaleString("en-US")} Bytes</p>
        </div>
      {/if}
    </div>
  {:else}
    <div class="file-list is-empty">
      <div class="file-list-message">
        {data.internationalization?.["wiki-page-file-no-files"]}
      </div>
    </div>
  {/if}

  {#if activeFileAction === "upload"}
    <FileUploadForm
      {data}
      onClose={() => (activeFileAction = null)}
      onUploaded={() => getFileList()}
    />
  {/if}

  {#if activeFileAction === "edit"}
    <form
      id="file-edit"
      class="file-edit"
      action="?/fileEdit"
      enctype="multipart/form-data"
      method="POST"
      use:editEnhance
    >
      <div class="file-form-field">
        <label for="file">
          {data.internationalization?.["wiki-page-file-upload.select"]}
        </label>
        <input
          name="file"
          class="file-attribute file"
          type="file"
          bind:files={$editFile}
        />
      </div>
      <div class="file-form-field">
        <label for="name">
          {data.internationalization?.["wiki-page-file-upload.name"]}
        </label>
        <input
          name="name"
          class="file-attribute name"
          placeholder={fileMap.get(fileEditId)?.name}
          type="text"
          bind:value={$editForm.name}
        />
      </div>
      <textarea
        name="comments"
        class="file-form-field file-comments"
        placeholder={data.internationalization?.["wiki-page-revision-comments"]}
        bind:value={$editForm.comments}></textarea>
      {#if pageLayoutState.current === Layout.WIKIDOT}
        <div class="buttons">
          <input
            class="btn btn-default"
            onclick={() => {
              editReset()
              activeFileAction = null
            }}
            type="button"
            value={data.internationalization?.cancel}
          />
          <input
            class="btn btn-primary"
            type="submit"
            value={data.internationalization?.save}
          />
        </div>
      {:else}
        <div class="action-row file-edit-actions">
          <button
            class="action-button file-edit-button button-cancel clickable"
            onclick={() => {
              editReset()
              activeFileAction = null
            }}
            type="button"
          >
            {data.internationalization?.cancel}
          </button>
          <button
            class="action-button file-edit-button button-save clickable"
            type="submit"
          >
            {data.internationalization?.save}
          </button>
        </div>
      {/if}
    </form>
  {/if}

  {#if activeFileAction === "move"}
    <form
      id="file-move"
      class="file-move"
      action="?/fileMove"
      method="POST"
      use:moveEnhance
    >
      <input
        name="destinationPage"
        class="file-move-destination-page"
        placeholder={data.internationalization?.["wiki-page-file-move-destination-page"]}
        type="text"
        bind:value={$moveForm.destinationPage}
      />
      <textarea
        name="comments"
        class="file-move-comments"
        placeholder={data.internationalization?.["wiki-page-revision-comments"]}
        bind:value={$moveForm.comments}></textarea>
      {#if pageLayoutState.current === Layout.WIKIDOT}
        <div class="buttons">
          <input
            class="btn btn-default"
            onclick={() => {
              moveReset()
              activeFileAction = null
            }}
            type="button"
            value={data.internationalization?.cancel}
          />
          <input
            class="btn btn-primary"
            type="submit"
            value={data.internationalization?.move}
          />
        </div>
      {:else}
        <div class="action-row file-move-actions">
          <button
            class="action-button file-move-button button-cancel clickable"
            onclick={() => {
              moveReset()
              activeFileAction = null
            }}
            type="button"
          >
            {data.internationalization?.cancel}
          </button>
          <button
            class="action-button file-move-button button-move clickable"
            type="submit"
          >
            {data.internationalization?.move}
          </button>
        </div>
      {/if}
    </form>
  {/if}

  {#if activeFileAction === "restore"}
    <form
      id="file-restore"
      class="file-restore"
      action="?/fileRestore"
      method="POST"
      use:restoreEnhance
    >
      <input
        name="newPage"
        class="file-restore-new-page"
        placeholder={data.internationalization?.["wiki-page-file-restore.new-page"]}
        type="text"
        bind:value={$restoreForm.newPage}
      />
      <input
        name="newName"
        class="file-restore-new-name"
        placeholder={data.internationalization?.["wiki-page-file-restore.new-name"]}
        type="text"
        bind:value={$restoreForm.newName}
      />
      <textarea
        name="comments"
        class="file-restore-comments"
        placeholder={data.internationalization?.["wiki-page-revision-comments"]}
        bind:value={$restoreForm.comments}></textarea>
      {#if pageLayoutState.current === Layout.WIKIDOT}
        <div class="buttons">
          <input
            class="btn btn-default"
            onclick={() => {
              restoreReset()
              activeFileAction = null
            }}
            type="button"
            value={data.internationalization?.cancel}
          />
          <input
            class="btn btn-primary"
            type="submit"
            value={data.internationalization?.restore}
          />
        </div>
      {:else}
        <div class="action-row file-restore-actions">
          <button
            class="action-button file-restore-button button-cancel clickable"
            onclick={() => {
              restoreReset()
              activeFileAction = null
            }}
            type="button"
          >
            {data.internationalization?.cancel}
          </button>
          <button
            class="action-button file-restore-button button-restore clickable"
            type="submit"
          >
            {data.internationalization?.restore}
          </button>
        </div>
      {/if}
    </form>
  {/if}

  {#if activeFileAction === "history"}
    <div class="revision-list">
      <div class="revision-header">
        <div class="revision-attribute action"></div>
        <div class="revision-attribute revision-number">
          {data.internationalization?.["wiki-page-revision-number"]}
        </div>
        <div class="revision-attribute revision-type">
          {data.internationalization?.["wiki-page-file-revision-type"]}
        </div>
        <div class="revision-attribute created-at">
          {data.internationalization?.["wiki-page-file.created-at"]}
        </div>
        <div class="revision-attribute user">
          {data.internationalization?.["wiki-page-revision-user"]}
        </div>
        <div class="revision-attribute page">
          {data.internationalization?.["wiki-page-file.page"]}
        </div>
        <div class="revision-attribute name">
          {data.internationalization?.["wiki-page-file.name"]}
        </div>
        <div class="revision-attribute mime">
          {data.internationalization?.["wiki-page-file.mime"]}
        </div>
        <div class="revision-attribute size">
          {data.internationalization?.["wiki-page-file.size"]}
        </div>
        <div class="revision-attribute comments">
          {data.internationalization?.["wiki-page-revision-comments"]}
        </div>
      </div>
      <!-- Here we sort the list in descending order. -->
      {#each [...fileRevisionMap].sort((a, b) => b[0] - a[0]) as [index, revisionItem] (index)}
        <div class="revision-row" data-id={revisionItem.revision_id}>
          <div class="revision-attribute action">
            {#if ["create", "regular"].includes(revisionItem.revision_type)}
              {#if pageLayoutState.current === Layout.WIKIDOT}
                <!-- svelte-ignore a11y_invalid_attribute -->
                <a
                  class="btn btn-primary btn-sm btn-small"
                  href="javascript:;"
                  onclick={() => {
                    fileEditId = revisionItem.file_id
                    rollbackFileRevision(revisionItem.revision_number)
                  }}
                >
                  {data.internationalization?.["wiki-page-revision-rollback"]}
                </a>
              {:else}
                <button
                  class="action-button revision-rollback clickable"
                  onclick={() => {
                    fileEditId = revisionItem.file_id
                    rollbackFileRevision(revisionItem.revision_number)
                  }}
                  type="button"
                >
                  {data.internationalization?.["wiki-page-revision-rollback"]}
                </button>
              {/if}
            {/if}
          </div>
          <div class="revision-attribute revision-number">
            {revisionItem.revision_number}
          </div>
          <div class="revision-attribute revision-type">
            {data.internationalization?.[
              `wiki-page-file-revision-type.${revisionItem.revision_type}`
            ]}
          </div>
          <div class="revision-attribute created-at">
            {new Date(revisionItem.created_at).toLocaleString()}
          </div>
          <div class="revision-attribute user">
            {revisionItem.user_id}
          </div>
          <div class="revision-attribute page">
            {revisionItem.page_id}
          </div>
          <div class="revision-attribute name">
            {revisionItem.name}
          </div>
          <div class="revision-attribute mime">
            {revisionItem.mime}
          </div>
          <div class="revision-attribute size">
            {revisionItem.size}
          </div>
          <div class="revision-attribute comments">
            {revisionItem.comments}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style lang="scss">
  .file-edit,
  .file-move,
  .file-restore {
    display: flex;
    flex-direction: column;
    gap: 15px;
    align-items: stretch;
    justify-content: stretch;
    width: 100%;
  }

  .file-panel {
    --files-line: color-mix(in srgb, currentColor 16%, transparent);
    --files-tint: color-mix(in srgb, currentColor 4%, transparent);
    --files-muted: color-mix(in srgb, currentColor 60%, transparent);
    --files-accent: #1f5fa8;
    --files-danger: #b3261e;
    --files-radius: var(--size-border-radius, 6px);
  }

  .file-panel > .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: flex-end;
    margin-bottom: 12px;

    input {
      min-height: 36px;
      padding: 6px 16px;
      font: inherit;
      font-weight: 600;
      color: inherit;
      cursor: pointer;
      background: transparent;
      border: 1px solid var(--files-line);
      border-radius: var(--files-radius);

      &:hover {
        background: var(--files-tint);
      }

      &.btn-primary {
        color: #fff;
        background: var(--files-accent);
        border-color: var(--files-accent);

        &:hover {
          background: color-mix(in srgb, var(--files-accent) 85%, #000);
        }
      }

      &:focus-visible {
        outline: 3px solid color-mix(in srgb, var(--files-accent) 55%, transparent);
        outline-offset: 2px;
      }
    }
  }

  // Rows are subgrids so every column lines up across rows, including actions.
  .file-list {
    display: grid;
    grid-template-columns: minmax(12rem, 1fr) auto auto auto auto;
    margin: 0 0 2em;
    border: 1px solid var(--files-line);
    border-radius: var(--files-radius);

    &.has-mime {
      grid-template-columns: minmax(12rem, 1fr) auto auto auto auto auto;
    }

    &.is-empty {
      display: block;
      padding: 24px 16px;
      color: var(--files-muted);
      text-align: center;
      border-style: dashed;
    }

    .file-list-header,
    .file-row {
      display: grid;
      grid-template-columns: subgrid;
      grid-column: 1 / -1;
      column-gap: 20px;
      align-items: center;
      padding: 0 14px;
    }

    .file-list-header {
      padding-block: 8px;
      font-size: 0.75em;
      font-weight: 600;
      color: var(--files-muted);
      text-transform: uppercase;
      letter-spacing: 0.06em;
      background: var(--files-tint);
      border-bottom: 1px solid var(--files-line);

      .file-attribute {
        font-size: inherit;
        color: inherit;
      }
    }

    .file-row {
      min-height: 48px;
      padding-block: 6px;

      + .file-row {
        border-top: 1px solid var(--files-line);
      }

      &:hover {
        background: var(--files-tint);
      }

      &.is-deleted .name a {
        color: var(--files-muted);
        text-decoration: line-through;
      }
    }

    .created-at,
    .updated-at,
    .mime {
      font-size: 0.9em;
      color: var(--files-muted);
      white-space: nowrap;
    }

    .size {
      font-variant-numeric: tabular-nums;
      text-align: right;
      white-space: nowrap;
    }

    .action {
      display: flex;
      gap: 2px;
      align-items: center;
      justify-content: flex-end;
    }
  }

  .file-attribute.name {
    display: flex;
    gap: 10px;
    align-items: center;
    min-width: 0;

    a {
      font-weight: 500;
      overflow-wrap: anywhere;
    }
  }

  .file-ext {
    flex: none;
    min-width: 2.75em;
    padding: 2px 4px;
    font-family: var(--font-mono, monospace);
    font-size: 0.7em;
    font-weight: 600;
    color: var(--files-muted);
    text-align: center;
    border: 1px solid var(--files-line);
    border-radius: 3px;
  }

  .file-empty {
    color: var(--files-muted);
  }

  // Wikidot row actions: quiet text buttons, Delete in red.
  .file-attribute.action > a,
  .file-information > summary {
    padding: 4px 8px;
    font-size: 0.9em;
    line-height: 1.4;
    color: var(--files-accent);
    white-space: nowrap;
    text-decoration: none;
    cursor: pointer;
    border-radius: var(--files-radius);

    &:hover,
    &:focus-visible {
      background: color-mix(in srgb, var(--files-accent) 10%, transparent);
    }

    &:focus-visible {
      outline: 2px solid var(--files-accent);
    }
  }

  .file-attribute.action > a.is-danger {
    color: var(--files-danger);

    &:hover,
    &:focus-visible {
      background: color-mix(in srgb, var(--files-danger) 10%, transparent);
    }
  }

  .file-list-footer {
    display: flex;
    flex-wrap: wrap;
    grid-column: 1 / -1;
    gap: 4px 16px;
    justify-content: space-between;
    padding: 8px 14px;
    font-size: 0.9em;
    color: var(--files-muted);
    background: var(--files-tint);
    border-top: 1px solid var(--files-line);

    p {
      margin: 0;
      font-variant-numeric: tabular-nums;
    }
  }

  .file-information {
    position: relative;

    summary {
      list-style: none;

      &::-webkit-details-marker {
        display: none;
      }
    }

    &[open] > summary {
      background: color-mix(in srgb, var(--files-accent) 10%, transparent);
    }

    .file-information-content {
      position: absolute;
      top: calc(100% + 6px);
      right: 0;
      z-index: 2;
      width: max-content;
      max-width: min(28rem, 80vw);
      padding: 14px 16px;
      color: #111;
      overflow-wrap: anywhere;
      background: #fff;
      border: 1px solid var(--files-line);
      border-radius: var(--files-radius);
      box-shadow: 0 6px 20px rgb(0 0 0 / 15%);

      h2 {
        margin: 0 0 8px;
        font-size: 1em;
      }
    }

    dl {
      display: grid;
      grid-template-columns: auto minmax(0, 1fr);
      gap: 4px 14px;
      margin: 0;
      font-size: 0.9em;
    }

    dt {
      font-weight: 600;
      color: var(--files-muted);
    }

    dd {
      margin: 0;
    }
  }

  // Narrow screens: each row becomes a card-like block; the header is dropped.
  @media (max-width: 40rem) {
    .file-list,
    .file-list.has-mime {
      display: block;

      .file-list-header {
        display: none;
      }

      .file-row {
        display: flex;
        flex-wrap: wrap;
        gap: 4px 12px;
        padding: 10px 14px;
      }

      .file-attribute.name {
        flex-basis: 100%;
      }

      .updated-at:has(.file-empty) {
        display: none;
      }

      .file-information-content {
        right: auto;
        left: 0;
      }

      .action {
        flex-basis: 100%;
        flex-wrap: wrap;
        justify-content: flex-start;
        margin-left: -8px;
      }
    }
  }

  .revision-list {
    display: table;
    width: 100%;

    .revision-header,
    .revision-row {
      display: table-row;

      .revision-attribute {
        display: table-cell;
      }
    }
  }
</style>
