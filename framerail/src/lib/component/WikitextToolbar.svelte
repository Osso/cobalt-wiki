<script lang="ts">
  import { tick } from "svelte"
  import { applyWikitextToolbar } from "$lib/wikitext-toolbar"

  let {
    textarea,
    value = $bindable("")
  }: {
    textarea: HTMLTextAreaElement | undefined
    value: string
  } = $props()

  const groups = [
    [
      ...Array.from({ length: 6 }, (_, index) => ({
        id: `heading${index + 1}`,
        label: `heading level ${index + 1}`,
        className: `weditor-h${index + 1}`
      })),
      { id: "bold", label: "bold", className: "weditor-bold" },
      { id: "italic", label: "italic", className: "weditor-italic" },
      { id: "underline", label: "underline", className: "weditor-underline" },
      { id: "strikethrough", label: "strikethrough", className: "weditor-strikethrough" },
      { id: "teletype", label: "teletype", className: "weditor-teletype" },
      { id: "quote", label: "quote", className: "weditor-quote" },
      { id: "superscript", label: "superscript", className: "weditor-superscript" },
      { id: "subscript", label: "subscript", className: "weditor-subscript" },
      { id: "raw", label: "raw", className: "weditor-raw" }
    ],
    [
      { id: "hr", label: "hr", className: "weditor-hr" },
      { id: "div", label: "div", className: "weditor-div" },
      { id: "clearFloat", label: "clear float", className: "weditor-clearfloat" },
      { id: "clearFloatLeft", label: "left", className: "weditor-clearfloatleft" },
      { id: "clearFloatRight", label: "right", className: "weditor-clearfloatright" },
      { id: "toc", label: "toc", className: "weditor-toc" },
      { id: "code", label: "code", className: "weditor-code" },
      { id: "uri", label: "url", className: "weditor-uri" },
      { id: "pageLink", label: "page link", className: "weditor-pagelink" },
      { id: "image", label: "image", className: "weditor-image" },
      { id: "html", label: "HTML", className: "weditor-html" }
    ],
    [
      { id: "numberedList", label: "numbered list", className: "weditor-numlist" },
      { id: "bulletedList", label: "bulleted list", className: "weditor-bullist" },
      {
        id: "increaseListIndent",
        label: "increase indent",
        className: "weditor-incindent"
      },
      {
        id: "decreaseListIndent",
        label: "decrease indent",
        className: "weditor-decindent"
      },
      { id: "definitionList", label: "definition list", className: "weditor-deflist" },
      { id: "footnote", label: "footnote", className: "weditor-footnote" },
      { id: "math", label: "math", className: "weditor-math" },
      { id: "inlineMath", label: "inline math", className: "weditor-mathinline" },
      { id: "bibliography", label: "bibliography", className: "weditor-bib" },
      { id: "bibliographycitation", label: "bibcite", className: "weditor-bibcite" }
    ]
  ]

  async function insert(control: string) {
    if (!textarea) return
    const scrollTop = textarea.scrollTop
    const result = applyWikitextToolbar(
      textarea.value,
      textarea.selectionStart,
      textarea.selectionEnd,
      control
    )
    value = result.value
    await tick()
    textarea.focus()
    textarea.setSelectionRange(result.start, result.end)
    textarea.scrollTop = scrollTop
  }
</script>

<div class="wikitext-toolbar" role="toolbar" aria-label="Wikitext formatting">
  {#each groups as group}
    <div class="toolbar-group">
      {#each group as control}
        <button
          type="button"
          class={control.className}
          disabled={!textarea}
          onpointerdown={(event) => event.preventDefault()}
          onclick={() => insert(control.id)}>{control.label}</button
        >
      {/each}
    </div>
  {/each}
</div>

<style>
  .wikitext-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .toolbar-group {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
  }

  button {
    min-height: 44px;
    padding: 4px 8px;
    color: inherit;
    background: transparent;
    border: 1px solid currentColor;
    cursor: pointer;
  }

  button:focus-visible {
    outline: 2px solid currentColor;
    outline-offset: 2px;
  }

  button:active {
    background: currentColor;
    color: Canvas;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
