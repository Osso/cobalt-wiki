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

  const stateLabels: Record<UploadRow["state"], string> = {
    queued: "Queued",
    uploading: "Uploading",
    uploaded: "Uploaded",
    failed: "Failed"
  }

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

  let dragging = $state(false)
  let totalSize = $derived(files.reduce((total, file) => total + file.size, 0))
  let settled = $derived(uploaded + failed)

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`
    const units = ["KB", "MB", "GB"]
    let value = bytes / 1024
    let unit = 0
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024
      unit += 1
    }
    return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`
  }

  function setSelection(selected: File[]) {
    files = selected
    rows = files.map((file) => ({ file, state: "queued" }))
    submitted = false
    formError = ""
    name = ""
  }

  function selectFiles(event: Event) {
    setSelection(Array.from((event.currentTarget as HTMLInputElement).files ?? []))
  }

  function dropFiles(event: DragEvent) {
    event.preventDefault()
    dragging = false
    const dropped = Array.from(event.dataTransfer?.files ?? [])
    if (uploading || dropped.length === 0) return
    setSelection(dropped)
  }

  function leaveDropzone(event: DragEvent) {
    const target = event.currentTarget as HTMLElement
    if (!target.contains(event.relatedTarget as Node | null)) dragging = false
  }

  function removeFile(index: number) {
    setSelection(files.filter((_, current) => current !== index))
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
  class:is-busy={uploading}
  action="?/fileUpload"
  enctype="multipart/form-data"
  method="POST"
  onsubmit={submitUpload}
>
  <label
    class="upload-dropzone"
    class:has-files={files.length > 0}
    class:is-dragging={dragging}
    for="file-upload-files"
    ondragleave={leaveDropzone}
    ondragover={(event) => {
      event.preventDefault()
      if (!uploading) dragging = true
    }}
    ondrop={dropFiles}
  >
    <input
      id="file-upload-files"
      name="file"
      class="file-attribute file upload-input"
      disabled={uploading}
      multiple
      onchange={selectFiles}
      type="file"
    />
    <svg class="upload-glyph" aria-hidden="true" viewBox="0 0 24 24">
      <path d="M12 15V4m0 0L7.5 8.5M12 4l4.5 4.5" />
      <path d="M4 14v4.5A1.5 1.5 0 0 0 5.5 20h13a1.5 1.5 0 0 0 1.5-1.5V14" />
    </svg>
    <span class="upload-prompt">
      <strong>{data.internationalization?.["wiki-page-file-upload.select"]}</strong>
      <span class="upload-hint">
        {#if files.length > 0}
          {files.length}
          {files.length === 1 ? "file" : "files"} · {formatSize(totalSize)} — drop or browse
          to replace
        {:else}
          Drop files here or <u>browse</u>. Several files upload one after another.
        {/if}
      </span>
    </span>
  </label>

  {#if rows.length > 0}
    <section class="upload-queue" aria-label="Upload queue">
      <header class="upload-summary">
        <p role="status">
          {#if submitted}
            <span class="summary-count">{uploaded}</span> uploaded,
            <span class="summary-count">{failed}</span> failed of
            <span class="summary-count">{rows.length}</span>
          {:else}
            <span class="summary-count">{rows.length}</span>
            {rows.length === 1 ? "file" : "files"} ready
          {/if}
        </p>
        {#if submitted}
          <div
            style:--done={uploaded / rows.length}
            style:--settled={settled / rows.length}
            class="upload-meter"
            aria-hidden="true"
          ></div>
        {/if}
      </header>
      <ul class="upload-results">
        {#each rows as row, index (index)}
          <li class="upload-row state-{row.state}" data-upload-index={index}>
            <span class="row-mark" aria-hidden="true"></span>
            <span class="row-body">
              <span class="row-name">{row.file.name}</span>
              <span class="row-size">{formatSize(row.file.size)}</span>
              {#if row.error}<span class="upload-error row-error">{row.error}</span>{/if}
            </span>
            <span class="row-state">{stateLabels[row.state]}</span>
            {#if !submitted && !uploading}
              <button
                class="row-remove"
                aria-label="Remove {row.file.name}"
                onclick={() => removeFile(index)}
                type="button">×</button
              >
            {/if}
            {#if row.state === "uploading"}<span class="row-progress" aria-hidden="true"
              ></span>{/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <div class="upload-fields">
    {#if files.length <= 1}
      <div class="file-form-field">
        <label for="file-upload-name"
          >{data.internationalization?.["wiki-page-file-upload.name"]}</label
        >
        <input
          id="file-upload-name"
          name="name"
          class="file-attribute name"
          disabled={uploading}
          placeholder={files[0]?.name}
          type="text"
          bind:value={name}
        />
      </div>
    {/if}
    <div class="file-form-field">
      <label for="file-upload-comments"
        >{data.internationalization?.["wiki-page-revision-comments"]}</label
      >
      <textarea
        id="file-upload-comments"
        name="comments"
        class="file-comments"
        disabled={uploading}
        rows="2"
        bind:value={comments}></textarea>
    </div>
  </div>

  {#if formError}<p class="upload-error form-error" role="alert">{formError}</p>{/if}
  {#if pageLayoutState.current === Layout.WIKIDOT}
    <div class="buttons">
      <input
        class="btn btn-default"
        disabled={uploading}
        onclick={onClose}
        type="button"
        value={submitted && files.length > 1
          ? "Close"
          : data.internationalization?.cancel}
      />
      <input
        class="btn btn-primary"
        disabled={uploading || submitted || files.length === 0}
        type="submit"
        value={data.internationalization?.upload}
      />
    </div>
  {:else}
    <div class="action-row file-upload-actions">
      <button
        class="action-button file-upload-button button-cancel clickable"
        disabled={uploading}
        onclick={onClose}
        type="button"
      >
        {submitted && files.length > 1 ? "Close" : data.internationalization?.cancel}
      </button>
      <button
        class="action-button file-upload-button button-upload clickable"
        disabled={uploading || submitted || files.length === 0}
        type="submit"
      >
        {data.internationalization?.upload}{files.length > 1
          ? ` ${files.length} files`
          : ""}
      </button>
    </div>
  {/if}
</form>

<style lang="scss">
  .file-upload {
    --upload-ink: currentColor;
    --upload-tint: color-mix(in srgb, currentColor 5%, transparent);
    --upload-line: color-mix(in srgb, currentColor 22%, transparent);
    --upload-muted: color-mix(in srgb, currentColor 62%, transparent);
    --upload-ok: #1d7a44;
    --upload-bad: #b3261e;
    --upload-busy: #1f5fa8;
    --upload-radius: var(--size-border-radius, 6px);

    display: flex;
    flex-direction: column;
    gap: 16px;
    align-items: stretch;
    width: 100%;
    padding-block: 8px;
  }

  // Drop zone: the whole label is the click and drop target for the hidden input.
  .upload-dropzone {
    position: relative;
    display: flex;
    gap: 16px;
    align-items: center;
    min-height: 88px;
    padding: 16px 20px;
    cursor: pointer;
    background: var(--upload-tint);
    border: 2px dashed var(--upload-line);
    border-radius: var(--upload-radius);
    transition:
      border-color 150ms ease-out,
      background-color 150ms ease-out;

    &:hover,
    &.is-dragging {
      background: color-mix(in srgb, var(--upload-busy) 8%, transparent);
      border-color: var(--upload-busy);
    }

    &:has(:focus-visible) {
      outline: 3px solid color-mix(in srgb, var(--upload-busy) 55%, transparent);
      outline-offset: 2px;
    }

    &.has-files {
      min-height: 64px;
      border-style: solid;
    }

    .is-busy & {
      cursor: progress;
      opacity: 0.6;
    }
  }

  .upload-input {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    white-space: nowrap;
    clip-path: inset(50%);
  }

  .upload-glyph {
    flex: none;
    width: 32px;
    height: 32px;
    fill: none;
    stroke: var(--upload-busy);
    stroke-width: 1.75;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .upload-prompt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;

    strong {
      font-weight: 600;
    }
  }

  .upload-hint {
    font-size: 0.9em;
    font-variant-numeric: tabular-nums;
    color: var(--upload-muted);
  }

  // Queue
  .upload-queue {
    overflow: hidden;
    border: 1px solid var(--upload-line);
    border-radius: var(--upload-radius);
  }

  .upload-summary {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 16px;
    align-items: center;
    justify-content: space-between;
    padding: 8px 14px;
    background: var(--upload-tint);
    border-bottom: 1px solid var(--upload-line);

    p {
      margin: 0;
      font-size: 0.9em;
    }
  }

  .summary-count {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  // Segmented meter: green for uploaded, red for failed, rest is pending.
  .upload-meter {
    flex: 1 1 120px;
    max-width: 240px;
    height: 6px;
    background:
      linear-gradient(
          to right,
          var(--upload-ok) 0 calc(var(--done) * 100%),
          var(--upload-bad) 0 calc(var(--settled) * 100%),
          transparent 0
        )
        no-repeat,
      var(--upload-line);
    border-radius: 3px;
  }

  .upload-results {
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .upload-row {
    position: relative;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    gap: 12px;
    align-items: center;
    min-height: 44px;
    padding: 6px 14px;

    & + & {
      border-top: 1px solid var(--upload-line);
    }
  }

  .row-mark {
    width: 10px;
    height: 10px;
    border: 2px solid var(--upload-muted);
    border-radius: 50%;

    .state-uploading & {
      border-color: var(--upload-busy);
      border-right-color: transparent;
      animation: upload-spin 700ms linear infinite;
    }

    .state-uploaded & {
      background: var(--upload-ok);
      border-color: var(--upload-ok);
    }

    .state-failed & {
      background: var(--upload-bad);
      border-color: var(--upload-bad);
      border-radius: 2px;
    }
  }

  .row-body {
    display: flex;
    flex-wrap: wrap;
    gap: 0 10px;
    align-items: baseline;
    min-width: 0;
  }

  .row-name {
    overflow-wrap: anywhere;
  }

  .row-size {
    font-size: 0.85em;
    font-variant-numeric: tabular-nums;
    color: var(--upload-muted);
  }

  .row-error {
    flex-basis: 100%;
    font-size: 0.85em;
  }

  .row-state {
    padding: 2px 8px;
    font-size: 0.75em;
    font-weight: 600;
    color: var(--upload-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    border: 1px solid currentColor;
    border-radius: 999px;

    .state-uploading & {
      color: var(--upload-busy);
    }

    .state-uploaded & {
      color: var(--upload-ok);
    }

    .state-failed & {
      color: var(--upload-bad);
    }
  }

  .row-remove {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    padding: 0;
    font-size: 1.25em;
    line-height: 1;
    color: var(--upload-muted);
    cursor: pointer;
    background: none;
    border: 0;
    border-radius: 50%;

    &:hover,
    &:focus-visible {
      color: var(--upload-bad);
      background: color-mix(in srgb, var(--upload-bad) 10%, transparent);
    }

    &:focus-visible {
      outline: 2px solid var(--upload-bad);
    }
  }

  // Indeterminate bar along the bottom edge of the active row.
  .row-progress {
    position: absolute;
    inset: auto 0 0;
    height: 2px;
    overflow: hidden;

    &::after {
      position: absolute;
      inset: 0;
      width: 40%;
      content: "";
      background: var(--upload-busy);
      animation: upload-sweep 1100ms ease-in-out infinite;
    }
  }

  .upload-fields {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px 16px;

    .file-form-field {
      display: flex;
      flex-direction: column;
      gap: 4px;
    }

    label {
      font-size: 0.9em;
      font-weight: 500;
    }

    input,
    textarea {
      box-sizing: border-box;
      width: 100%;
      min-height: 40px;
      padding: 8px 10px;
      font: inherit;
      border: 1px solid var(--upload-line);
      border-radius: var(--upload-radius);

      &:focus-visible {
        outline: 2px solid var(--upload-busy);
        outline-offset: 1px;
      }
    }

    textarea {
      resize: vertical;
    }
  }

  .upload-error {
    color: var(--upload-bad);
  }

  .form-error {
    padding: 8px 12px;
    margin: 0;
    background: color-mix(in srgb, var(--upload-bad) 8%, transparent);
    border-left: 3px solid var(--upload-bad);
  }

  .buttons,
  .file-upload-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: flex-end;
  }

  .buttons input,
  .file-upload-actions button {
    min-width: 96px;
    min-height: 40px;
    padding: 8px 18px;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    border-radius: var(--upload-radius);

    &:focus-visible {
      outline: 3px solid color-mix(in srgb, var(--upload-busy) 55%, transparent);
      outline-offset: 2px;
    }

    &:disabled {
      cursor: not-allowed;
      opacity: 0.5;
    }
  }

  .buttons .btn-default,
  .file-upload-actions .button-cancel {
    color: inherit;
    background: transparent;
    border: 1px solid var(--upload-line);

    &:hover:not(:disabled) {
      background: var(--upload-tint);
    }
  }

  .buttons .btn-primary,
  .file-upload-actions .button-upload {
    color: #fff;
    background: var(--upload-busy);
    border: 1px solid var(--upload-busy);

    &:hover:not(:disabled) {
      background: color-mix(in srgb, var(--upload-busy) 85%, #000);
    }
  }

  @keyframes upload-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes upload-sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(250%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .row-mark,
    .row-progress::after {
      animation: none;
    }
  }
</style>
