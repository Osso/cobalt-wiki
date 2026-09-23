<script lang="ts">
  import { onMount, untrack } from "svelte"
  import {
    extractEquations,
    normalizeFlickrSource,
    type WizardKind,
    type WizardOptions
  } from "$lib/wikitext-wizards"

  interface Props {
    kind: WizardKind
    source: string
    onInsert: (options: WizardOptions) => void | Promise<void>
    onCancel: () => void
    pageLookup?: (query: string) => Promise<{ slug: string; title: string }[]>
    attachmentLookup?: () => Promise<{ name: string; url: string }[]>
  }

  let { kind, source, onInsert, onCancel, pageLookup, attachmentLookup }: Props = $props()
  let dialog: HTMLDialogElement
  let rows = $state(3)
  let columns = $state(3)
  let headers = $state(false)
  let codeType = $state("")
  let uri = $state("http://")
  let anchor = $state("")
  let newWindow = $state(false)
  let page = $state("")
  let pageMatches = $state<{ slug: string; title: string }[]>([])
  let pagePending = $state(false)
  let pageError = $state("")
  let imageSource = $state<"uri" | "file" | "flickr">("uri")
  let imageUri = $state("")
  let flickr = $state("")
  let file = $state("")
  let files = $state<{ name: string; url: string }[]>([])
  let filePending = $state(false)
  let fileError = $state("")
  let position = $state<"" | "l" | "r" | "c" | "fl" | "fr">("")
  let previewUri = $state("")
  let previewStatus = $state("")
  const equations = $derived(extractEquations(source))
  let label = $state(untrack(() => equations[0]?.label ?? ""))
  let withEq = $state(true)
  let error = $state("")
  let inserting = $state(false)
  const selectedEquation = $derived(
    equations.find((equation) => equation.label === label)
  )

  onMount(() => {
    dialog.showModal()
    dialog.querySelector<HTMLElement>("input, select")?.focus()
  })

  $effect(() => {
    if (kind !== "pageLink" || !pageLookup || page.trim().length < 2) {
      pageMatches = []
      pagePending = false
      pageError = ""
      return
    }
    const query = page.trim()
    let current = true
    pageMatches = []
    pageError = ""
    pagePending = true
    const timer = setTimeout(async () => {
      try {
        const matches = await pageLookup(query)
        if (current) pageMatches = matches
      } catch {
        if (current) {
          pageError = "Unable to look up pages. You can still enter a page name."
        }
      } finally {
        if (current) pagePending = false
      }
    }, 500)
    return () => {
      current = false
      clearTimeout(timer)
    }
  })

  $effect(() => {
    if (kind !== "image" || imageSource !== "file" || !attachmentLookup) return
    let current = true
    filePending = true
    fileError = ""
    attachmentLookup()
      .then(
        (attachments) => {
          if (current) files = attachments
        },
        () => {
          if (current) fileError = "Unable to load attached files."
        }
      )
      .finally(() => {
        if (current) filePending = false
      })
    return () => {
      current = false
    }
  })

  function checkImage() {
    previewUri = ""
    previewStatus = ""
    try {
      const url = new URL(imageUri)
      if (url.protocol !== "http:" && url.protocol !== "https:") {
        throw new Error("Enter an HTTP or HTTPS image URL.")
      }
      previewUri = url.href
      previewStatus = "Loading image…"
    } catch {
      previewStatus = "Enter a valid HTTP or HTTPS image URL."
    }
  }

  function imageValue(): string {
    switch (imageSource) {
      case "file":
        return file
      case "flickr":
        return flickr
      case "uri":
        return imageUri
    }
  }

  function wizardOptions(): WizardOptions {
    switch (kind) {
      case "table": {
        const validRows = Number.isInteger(rows) && rows >= 1 && rows <= 99
        const validColumns = Number.isInteger(columns) && columns >= 1 && columns <= 99
        if (!validRows || !validColumns) {
          throw new Error("Rows and columns must be whole numbers from 1 to 99.")
        }
        return { kind, rows, columns, headers }
      }
      case "code":
        return { kind, type: codeType }
      case "uri":
        return { kind, uri, anchor, newWindow }
      case "pageLink":
        return { kind, page, anchor }
      case "image": {
        const value = imageValue().trim()
        if (!value) throw new Error("Choose or enter an image source.")
        if (imageSource === "flickr") normalizeFlickrSource(value)
        return { kind, source: imageSource, value, position }
      }
      case "eref":
        if (!selectedEquation) throw new Error("No labelled equations found.")
        return { kind, label: selectedEquation.label, withEq }
    }
  }

  async function insert(event: SubmitEvent) {
    event.preventDefault()
    if (inserting) return
    error = ""
    inserting = true
    try {
      await onInsert(wizardOptions())
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Unable to insert code."
    } finally {
      inserting = false
    }
  }
</script>

<dialog
  bind:this={dialog}
  aria-label={{
    table: "Table wizard",
    code: "Code block wizard",
    uri: "URL link wizard",
    pageLink: "Page link wizard",
    image: "Image wizard",
    eref: "Equation reference wizard"
  }[kind]}
  oncancel={(event) => {
    event.preventDefault()
    onCancel()
  }}
>
  <form onsubmit={insert}>
    {#if kind === "table"}
      <p>This wizard will create an empty table with the specified properties:</p>
      <label
        >Number of rows: <input
          max="99"
          min="1"
          required
          type="number"
          bind:value={rows}
        /></label
      >
      <label
        >Number of columns: <input
          max="99"
          min="1"
          required
          type="number"
          bind:value={columns}
        /></label
      >
      <label><input type="checkbox" bind:checked={headers} /> First row as header</label>
    {:else if kind === "code"}
      <p>
        This wizard will create a code block. Choose a language for syntax highlighting.
      </p>
      <label
        >Code type:
        <select bind:value={codeType}>
          <option value="">not specified</option><option value="Cpp">C++</option>
          <option value="CSS">CSS</option><option value="PHP">PHP</option>
          <option value="HTML">HTML &amp; XHTML</option><option value="diff">Diff</option>
          <option value="Java">Java</option><option value="JavaScipt">JavaScript</option>
          <option value="Perl">Perl</option><option value="Python">Python</option>
          <option value="Ruby">Ruby</option><option value="SQL">SQL</option>
          <option value="XML">XML</option>
        </select>
      </label>
    {:else if kind === "uri"}
      <p>This wizard will create a URL link:</p>
      <label>URL: <input required type="text" bind:value={uri} /></label>
      <label>Anchor text: <input type="text" bind:value={anchor} /></label>
      <label
        ><input type="checkbox" bind:checked={newWindow} /> Open in a new window</label
      >
    {:else if kind === "pageLink"}
      <p>
        Enter the destination page name. A missing page can still be linked for later
        creation.
      </p>
      <label
        >Page name: <input
          autocomplete="off"
          required
          type="text"
          bind:value={page}
        /></label
      >
      {#if pagePending}<p role="status">Looking up pages…</p>{/if}
      {#if pageError}<p role="alert">{pageError}</p>{/if}
      {#if pageMatches.length}
        <ul aria-label="Matching pages">
          {#each pageMatches as match (match.slug)}
            <li>
              <button
                onclick={() => {
                  page = match.slug
                  pageMatches = []
                }}
                type="button">{match.slug} ({match.title})</button
              >
            </li>
          {/each}
        </ul>
      {/if}
      <label>Anchor text (optional): <input type="text" bind:value={anchor} /></label>
    {:else if kind === "image"}
      <p>This wizard will help you insert an image into the page.</p>
      <fieldset>
        <legend>Source type:</legend>
        <label
          ><input type="radio" value="uri" bind:group={imageSource} /> external image (via URL)</label
        >
        {#if attachmentLookup}<label
            ><input type="radio" value="file" bind:group={imageSource} /> attached file</label
          >{/if}
        <label
          ><input type="radio" value="flickr" bind:group={imageSource} /> Flickr.com</label
        >
      </fieldset>
      {#if imageSource === "uri"}
        <label>Image URL: <input type="text" bind:value={imageUri} /></label>
        <button onclick={checkImage} type="button">Check image</button>
        {#if previewUri}<img
            alt="Preview of URL"
            onerror={() => (previewStatus = "Image unavailable.")}
            onload={() => (previewStatus = "Image loaded.")}
            src={previewUri}
          />{/if}
        {#if previewStatus}<p role="status">{previewStatus}</p>{/if}
      {:else if imageSource === "file"}
        {#if filePending}<p role="status">Loading attached files…</p>{/if}
        {#if fileError}<p role="alert">{fileError}</p>{/if}
        {#if !filePending && !fileError}
          {#if files.length}
            <label
              >Attached file: <select bind:value={file}
                ><option value="">Choose a file</option
                >{#each files as attachment (attachment.name)}<option
                    value={attachment.name}>{attachment.name}</option
                  >{/each}</select
              ></label
            >
            {#if file}<img
                alt="Selected attachment"
                src={files.find((attachment) => attachment.name === file)?.url}
              />{/if}
          {:else}<p>No attached files available.</p>{/if}
        {/if}
      {:else}
        <p>Enter a public Flickr page URL, numeric image ID, or static image URL.</p>
        <label>Flickr image: <input type="text" bind:value={flickr} /></label>
      {/if}
      <label
        >Position:
        <select bind:value={position}>
          <option value="">no position</option><option value="l">align left</option>
          <option value="r">align right</option><option value="c">center</option>
          <option value="fl">left with text wrapping (float)</option>
          <option value="fr">right with text wrapping (float)</option>
        </select>
      </label>
    {:else if kind === "eref"}
      {#if equations.length}
        <label
          >Please select equation label:
          <select bind:value={label}
            >{#each equations as equation (equation.label)}<option value={equation.label}
                >{equation.label}</option
              >{/each}</select
          >
        </label>
        <fieldset>
          <legend>Select output:</legend>
          <label
            ><input type="radio" value={true} bind:group={withEq} /> Eq.(number)</label
          >
          <label
            ><input type="radio" value={false} bind:group={withEq} /> just number</label
          >
        </fieldset>
        <h3>Equation source preview:</h3>
        <pre>{selectedEquation?.source}</pre>
      {:else}<p role="status">Sorry, no labelled equations found.</p>{/if}
    {/if}
    {#if error}<p role="alert">{error}</p>{/if}
    <div class="actions">
      <button onclick={onCancel} type="button">Cancel</button>
      <button disabled={inserting || (kind === "eref" && !equations.length)} type="submit"
        >Insert code</button
      >
    </div>
  </form>
</dialog>

<style>
  dialog {
    max-width: min(36rem, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    padding: 1.25rem;
    overflow: auto;
    color: #222;
    background: #fff;
    border: 1px solid #888;
  }
  form {
    display: grid;
    gap: 0.75rem;
  }
  label {
    display: block;
  }
  input:not([type="checkbox"]):not([type="radio"]),
  select {
    box-sizing: border-box;
    max-width: 100%;
    font: inherit;
  }
  input[type="text"] {
    width: min(100%, 26rem);
  }
  input[type="number"] {
    width: 5rem;
  }
  fieldset {
    border: 1px solid #aaa;
  }
  fieldset label {
    margin: 0.35rem 0;
  }
  button {
    min-height: 2.75rem;
    font: inherit;
    cursor: pointer;
  }
  :is(button, input, select):focus-visible {
    outline: 2px solid #165c9a;
    outline-offset: 2px;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }
  img {
    display: block;
    max-width: 100%;
    max-height: 15rem;
    object-fit: contain;
  }
  pre {
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
  [role="alert"] {
    color: #a11;
  }
  p {
    margin: 0;
  }
</style>
