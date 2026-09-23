<script lang="ts">
  import { tick } from "svelte"
  import { applyWikitextToolbar } from "$lib/wikitext-toolbar"
  import { applyWizard, type WizardKind, type WizardOptions } from "$lib/wikitext-wizards"
  import WikitextWizard from "$lib/component/WikitextWizard.svelte"

  let {
    textarea,
    value = $bindable(),
    pageLookup,
    attachmentLookup
  }: {
    textarea: HTMLTextAreaElement | undefined
    value: string | undefined
    pageLookup?: (query: string) => Promise<{ slug: string; title: string }[]>
    attachmentLookup?: () => Promise<{ name: string; url: string }[]>
  } = $props()

  let wizard = $state<{
    kind: WizardKind
    source: string
    start: number
    end: number
    scrollTop: number
  } | null>(null)

  const wizardControls: Record<string, WizardKind> = {
    tableWizard: "table",
    codeWizard: "code",
    uriWizard: "uri",
    pageLinkWizard: "pageLink",
    imageWizard: "image",
    erefWizard: "eref"
  }

  function openWizard(kind: WizardKind) {
    if (!textarea) return
    wizard = {
      kind,
      source: value ?? textarea.value,
      start: textarea.selectionStart,
      end: textarea.selectionEnd,
      scrollTop: textarea.scrollTop
    }
  }

  async function closeWizard(options?: WizardOptions) {
    if (!wizard || !textarea) return
    const captured = wizard
    const result = options
      ? applyWizard(captured.source, captured.start, captured.end, options)
      : null
    if (result) value = result.value
    wizard = null
    await tick()
    textarea.focus()
    textarea.setSelectionRange(
      result?.start ?? captured.start,
      result?.end ?? captured.end
    )
    textarea.scrollTop = captured.scrollTop
  }

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
      {
        id: "tableWizard",
        label: "table wizard",
        className: "weditor-table",
        offset: 440
      },
      { id: "toc", label: "table of contents", className: "weditor-toc", offset: 462 },
      { id: "code", label: "code", className: "weditor-code", offset: 660 },
      {
        id: "codeWizard",
        label: "code block wizard",
        className: "weditor-codewiz",
        offset: 682
      },
      { id: "uri", label: "url", className: "weditor-uri", offset: 484 },
      {
        id: "uriWizard",
        label: "URL link wizard",
        className: "weditor-uriwiz",
        offset: 506
      },
      { id: "pageLink", label: "page link", className: "weditor-pagelink", offset: 528 },
      {
        id: "pageLinkWizard",
        label: "page link wizard",
        className: "weditor-pagelinkwiz",
        offset: 550
      },
      { id: "image", label: "image", className: "weditor-image", offset: 572 },
      {
        id: "imageWizard",
        label: "image wizard",
        className: "weditor-imagewiz",
        offset: 594
      },
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
        id: "erefWizard",
        label: "equation reference",
        className: "weditor-eqref",
        offset: 880
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
    const selectedWizard = wizardControls[control]
    if (selectedWizard) {
      openWizard(selectedWizard)
      return
    }
    const scrollTop = textarea.scrollTop
    const result = applyWikitextToolbar(
      value ?? textarea.value,
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

<div class="wikitext-toolbar" aria-label="Wikitext formatting" role="toolbar">
  {#each groups as group, row (group[0].id)}
    <div class="toolbar-group">
      {#if row === 0}
        <div class="submenu-holder">
          <button
            style:background-position="0 0"
            class={headings[0].className}
            aria-label={headings[0].label}
            disabled={!textarea}
            onclick={() => insert(headings[0].id)}
            onpointerdown={(event) => event.preventDefault()}
            title={headings[0].label}
            type="button"
          ></button>
          <div class="submenu" aria-label="Other heading levels">
            {#each headings.slice(1) as heading (heading.id)}
              <button
                style:background-position={`-${heading.offset}px 0`}
                class={heading.className}
                aria-label={heading.label}
                disabled={!textarea}
                onclick={() => insert(heading.id)}
                onpointerdown={(event) => event.preventDefault()}
                title={heading.label}
                type="button"
              ></button>
            {/each}
          </div>
        </div>
      {/if}
      {#each group as control, index (control.id)}
        {#if row === 1 && index === 2}
          <div class="submenu-holder">
            <button
              style:background-position={`-${clearFloat[0].offset}px 0`}
              class={clearFloat[0].className}
              aria-label={clearFloat[0].label}
              disabled={!textarea}
              onclick={() => insert(clearFloat[0].id)}
              onpointerdown={(event) => event.preventDefault()}
              title={clearFloat[0].label}
              type="button"
            ></button>
            <div class="submenu" aria-label="Clear float direction">
              {#each clearFloat.slice(1) as direction (direction.id)}
                <button
                  style:background-position={`-${direction.offset}px 0`}
                  class={direction.className}
                  aria-label={direction.label}
                  disabled={!textarea}
                  onclick={() => insert(direction.id)}
                  onpointerdown={(event) => event.preventDefault()}
                  title={direction.label}
                  type="button"
                ></button>
              {/each}
            </div>
          </div>
        {/if}
        {#if (row === 1 && ["code", "uri", "image", "html"].includes(control.id)) || (row === 2 && ["math", "bibliography"].includes(control.id))}
          <span class="separator" aria-hidden="true"></span>
        {/if}
        <button
          style:background-position={`-${control.offset}px 0`}
          class={control.className}
          aria-label={control.label}
          disabled={!textarea}
          onclick={() => insert(control.id)}
          onpointerdown={(event) => event.preventDefault()}
          title={control.label}
          type="button"
        ></button>
      {/each}
    </div>
  {/each}
</div>

{#if wizard}
  <WikitextWizard
    kind={wizard.kind}
    source={wizard.source}
    {pageLookup}
    {attachmentLookup}
    onInsert={(options) => closeWizard(options)}
    onCancel={() => closeWizard()}
  />
{/if}

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
    cursor: pointer;
    background-color: transparent;
    background-image: url("/cobalt-editor/icons1.png");
    background-repeat: no-repeat;
    border: 0;
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
    cursor: not-allowed;
    opacity: 0.5;
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
    top: 100%;
    left: 0;
    z-index: 1;
    display: none;
    background: #fff;
    box-shadow: 0 1px 3px #888;
  }

  .submenu-holder:hover .submenu,
  .submenu-holder:focus-within .submenu {
    display: block;
  }
</style>
