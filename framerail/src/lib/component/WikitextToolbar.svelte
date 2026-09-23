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

  // Offsets measured from the source editor's 22px icons1.png sprite.
  const headings = Array.from({ length: 6 }, (_, index) => ({
    id: `heading${index + 1}`,
    label: `heading level ${index + 1}`,
    className: `weditor-h${index + 1}`,
    offset: index * 22
  }))

  const groups = [
    [
      { id: "bold", label: "bold", className: "weditor-bold", offset: 132 },
      { id: "italic", label: "italic", className: "weditor-italic", offset: 154 },
      {
        id: "underline",
        label: "underline",
        className: "weditor-underline",
        offset: 176
      },
      {
        id: "strikethrough",
        label: "strikethrough",
        className: "weditor-strikethrough",
        offset: 198
      },
      { id: "teletype", label: "teletype", className: "weditor-teletype", offset: 220 },
      { id: "quote", label: "quote", className: "weditor-quote", offset: 242 },
      {
        id: "superscript",
        label: "superscript",
        className: "weditor-superscript",
        offset: 264
      },
      {
        id: "subscript",
        label: "subscript",
        className: "weditor-subscript",
        offset: 286
      },
      { id: "raw", label: "raw", className: "weditor-raw", offset: 308 }
    ],
    [
      { id: "hr", label: "horizontal rule", className: "weditor-hr", offset: 330 },
      { id: "div", label: "div", className: "weditor-div", offset: 352 },
      { id: "toc", label: "table of contents", className: "weditor-toc", offset: 462 },
      { id: "code", label: "code", className: "weditor-code", offset: 660 },
      { id: "uri", label: "url", className: "weditor-uri", offset: 484 },
      { id: "pageLink", label: "page link", className: "weditor-pagelink", offset: 528 },
      { id: "image", label: "image", className: "weditor-image", offset: 572 },
      { id: "html", label: "HTML", className: "weditor-html", offset: 946 }
    ],
    [
      {
        id: "numberedList",
        label: "numbered list",
        className: "weditor-numlist",
        offset: 704
      },
      {
        id: "bulletedList",
        label: "bulleted list",
        className: "weditor-bullist",
        offset: 726
      },
      {
        id: "increaseListIndent",
        label: "increase indent",
        className: "weditor-incindent",
        offset: 748
      },
      {
        id: "decreaseListIndent",
        label: "decrease indent",
        className: "weditor-decindent",
        offset: 770
      },
      {
        id: "definitionList",
        label: "definition list",
        className: "weditor-deflist",
        offset: 792
      },
      { id: "footnote", label: "footnote", className: "weditor-footnote", offset: 814 },
      { id: "math", label: "math", className: "weditor-math", offset: 836 },
      {
        id: "inlineMath",
        label: "inline math",
        className: "weditor-mathinline",
        offset: 858
      },
      {
        id: "bibliography",
        label: "bibliography",
        className: "weditor-bib",
        offset: 902
      },
      {
        id: "bibliographycitation",
        label: "bibcite",
        className: "weditor-bibcite",
        offset: 924
      }
    ]
  ]

  const clearFloat = [
    {
      id: "clearFloat",
      label: "clear float",
      className: "weditor-clearfloat",
      offset: 374
    },
    {
      id: "clearFloatLeft",
      label: "clear float left",
      className: "weditor-clearfloatleft",
      offset: 396
    },
    {
      id: "clearFloatRight",
      label: "clear float right",
      className: "weditor-clearfloatright",
      offset: 418
    }
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
  {#each groups as group, row}
    <div class="toolbar-group">
      {#if row === 0}
        <div class="submenu-holder">
          <button
            type="button"
            class={headings[0].className}
            style:background-position="0 0"
            aria-label={headings[0].label}
            title={headings[0].label}
            disabled={!textarea}
            onpointerdown={(event) => event.preventDefault()}
            onclick={() => insert(headings[0].id)}
          ></button>
          <div class="submenu" aria-label="Other heading levels">
            {#each headings.slice(1) as heading}
              <button
                type="button"
                class={heading.className}
                style:background-position={`-${heading.offset}px 0`}
                aria-label={heading.label}
                title={heading.label}
                disabled={!textarea}
                onpointerdown={(event) => event.preventDefault()}
                onclick={() => insert(heading.id)}
              ></button>
            {/each}
          </div>
        </div>
      {/if}
      {#each group as control, index}
        {#if row === 1 && index === 2}
          <div class="submenu-holder">
            <button
              type="button"
              class={clearFloat[0].className}
              style:background-position={`-${clearFloat[0].offset}px 0`}
              aria-label={clearFloat[0].label}
              title={clearFloat[0].label}
              disabled={!textarea}
              onpointerdown={(event) => event.preventDefault()}
              onclick={() => insert(clearFloat[0].id)}
            ></button>
            <div class="submenu" aria-label="Clear float direction">
              {#each clearFloat.slice(1) as direction}
                <button
                  type="button"
                  class={direction.className}
                  style:background-position={`-${direction.offset}px 0`}
                  aria-label={direction.label}
                  title={direction.label}
                  disabled={!textarea}
                  onpointerdown={(event) => event.preventDefault()}
                  onclick={() => insert(direction.id)}
                ></button>
              {/each}
            </div>
          </div>
        {/if}
        {#if (row === 1 && (index === 3 || index === 4 || index === 6 || index === 7)) || (row === 2 && (index === 6 || index === 8))}
          <span class="separator" aria-hidden="true"></span>
        {/if}
        <button
          type="button"
          class={control.className}
          style:background-position={`-${control.offset}px 0`}
          aria-label={control.label}
          title={control.label}
          disabled={!textarea}
          onpointerdown={(event) => event.preventDefault()}
          onclick={() => insert(control.id)}
        ></button>
      {/each}
    </div>
  {/each}
</div>

<style>
  .wikitext-toolbar {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }

  .toolbar-group {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    align-items: center;
  }

  button {
    display: block;
    width: 22px;
    height: 22px;
    padding: 0;
    background-color: transparent;
    background-image: url("/cobalt-editor/icons1.png");
    background-repeat: no-repeat;
    border: 0;
    cursor: pointer;
  }

  button:hover,
  button:active {
    background-color: #ddd;
  }

  button:focus-visible {
    outline: 2px solid #333;
    outline-offset: 1px;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .separator {
    width: 10px;
    height: 22px;
  }

  .submenu-holder {
    position: relative;
  }

  .submenu {
    position: absolute;
    z-index: 1;
    top: 100%;
    left: 0;
    display: none;
    background: #fff;
    box-shadow: 0 1px 3px #888;
  }

  .submenu-holder:hover .submenu,
  .submenu-holder:focus-within .submenu {
    display: block;
  }
</style>
