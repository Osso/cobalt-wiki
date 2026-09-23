<script lang="ts">
  import { onMount } from "svelte"
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
  let label = $state("")
  let withEq = $state(true)
  let error = $state("")
  let inserting = $state(false)
  const equations = $derived(extractEquations(source))
  const selectedEquation = $derived(
    equations.find((equation) => equation.label === (label || equations[0]?.label))
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
        if (current)
          pageError = "Unable to look up pages. You can still enter a page name."
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

  async function insert(event: SubmitEvent) {
    event.preventDefault()
    if (inserting) return
    error = ""
    inserting = true
    try {
      switch (kind) {
        case "table":
          if (
            !Number.isInteger(rows) ||
            rows < 1 ||
            rows > 99 ||
            !Number.isInteger(columns) ||
            columns < 1 ||
            columns > 99
          ) {
            throw new Error("Rows and columns must be whole numbers from 1 to 99.")
          }
          await onInsert({ kind, rows, columns, headers })
          return
        case "code":
          await onInsert({ kind, type: codeType })
          return
        case "uri":
          await onInsert({ kind, uri, anchor, newWindow })
          return
        case "pageLink":
          await onInsert({ kind, page, anchor })
          return
        case "image": {
          const value =
            imageSource === "file" ? file : imageSource === "flickr" ? flickr : imageUri
          if (!value.trim()) throw new Error("Choose or enter an image source.")
          if (imageSource === "flickr") normalizeFlickrSource(value.trim())
          await onInsert({ kind, source: imageSource, value: value.trim(), position })
          return
        }
        case "eref":
          if (!selectedEquation) throw new Error("No labelled equations found.")
          await onInsert({ kind, label: selectedEquation.label, withEq })
          return
      }
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
          type="number"
          min="1"
          max="99"
          required
          bind:value={rows}
        /></label
      >
      <label
        >Number of columns: <input
          type="number"
          min="1"
          max="99"
          required
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
      <label>URL: <input type="text" required bind:value={uri} /></label>
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
          type="text"
          required
          autocomplete="off"
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
                type="button"
                onclick={() => {
                  page = match.slug
                  pageMatches = []
                }}>{match.slug} ({match.title})</button
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
        <button type="button" onclick={checkImage}>Check image</button>
        {#if previewUri}<img
            src={previewUri}
            alt="Preview of URL"
            onload={() => (previewStatus = "Image loaded.")}
            onerror={() => (previewStatus = "Image unavailable.")}
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
                src={files.find((attachment) => attachment.name === file)?.url}
                alt="Selected attachment"
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
            ><input type="radio" bind:group={withEq} value={true} /> Eq.(number)</label
          >
          <label
            ><input type="radio" bind:group={withEq} value={false} /> just number</label
          >
        </fieldset>
        <h3>Equation source preview:</h3>
        <pre>{selectedEquation?.source}</pre>
      {:else}<p role="status">Sorry, no labelled equations found.</p>{/if}
    {/if}
    {#if error}<p role="alert">{error}</p>{/if}
    <div class="actions">
      <button type="button" onclick={onCancel}>Cancel</button>
      <button type="submit" disabled={inserting || (kind === "eref" && !equations.length)}
        >Insert code</button
      >
    </div>
  </form>
</dialog>

<style>
  dialog {
    max-width: min(36rem, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    overflow: auto;
    padding: 1.25rem;
    border: 1px solid #888;
    background: #fff;
    color: #222;
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
    justify-content: flex-end;
    gap: 0.5rem;
  }
  img {
    display: block;
    max-width: 100%;
    max-height: 15rem;
    object-fit: contain;
  }
  pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  [role="alert"] {
    color: #a11;
  }
  p {
    margin: 0;
  }
</style>
