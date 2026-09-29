<script lang="ts">
  import { tick } from "svelte"
  import { applyWikitextToolbar, type ToolbarResult } from "$lib/wikitext-toolbar"
  import { applyEnterAssist, shortcutControl } from "$lib/wikitext-keyboard"
  import { applyWizard, type WizardKind, type WizardOptions } from "$lib/wikitext-wizards"
  import WikitextWizard from "$lib/component/WikitextWizard.svelte"
  import LucideIcon from "$lib/component/LucideIcon.svelte"
  import { toolbarRows, type ToolbarControl } from "$lib/wikitext-toolbar-layout"

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

  $effect(() => {
    const target = textarea
    if (!target) return
    const keyDown = (event: KeyboardEvent) => {
      const control = shortcutControl(event)
      if (!control) return
      event.preventDefault()
      if (control === "tab") event.stopPropagation()
      void insert(control)
    }
    const keyUp = (event: KeyboardEvent) => {
      if (event.key !== "Enter" || event.isComposing) return
      const result = applyEnterAssist(target.value, target.selectionStart)
      if (result.value !== target.value) void applyResult(result, target.scrollTop)
    }
    target.addEventListener("keydown", keyDown)
    target.addEventListener("keyup", keyUp)
    return () => {
      target.removeEventListener("keydown", keyDown)
      target.removeEventListener("keyup", keyUp)
    }
  })

  async function applyResult(result: ToolbarResult, scrollTop: number) {
    const target = textarea
    if (!target) return
    value = result.value
    await tick()
    target.focus()
    target.setSelectionRange(result.start, result.end)
    target.scrollTop = scrollTop
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
    await applyResult(result, scrollTop)
  }
</script>

{#snippet control(item: ToolbarControl, extra = "")}
  <button
    class="tool {extra}"
    aria-label={item.label}
    disabled={!textarea}
    onclick={() => insert(item.id)}
    onpointerdown={(event) => event.preventDefault()}
    title={item.label}
    type="button"
  >
    {#if item.icon}
      <LucideIcon icon={item.icon} size="18px" />
    {:else}
      <span class="text">{@html item.text}</span>
    {/if}
  </button>
{/snippet}

<div class="wikitext-toolbar" aria-label="Wikitext formatting" role="toolbar">
  {#each toolbarRows as row, index (index)}
    <div class="toolbar-group">
      {#each row as item ("main" in item ? item.main.id : item.id)}
        {#if !("main" in item)}
          {@render control(item)}
        {:else if "wizard" in item}
          <span class="split">
            {@render control(item.main)}
            {@render control(item.wizard, "wizard")}
          </span>
        {:else}
          <div class="submenu-holder">
            {@render control(item.main, "has-menu")}
            <div class="submenu" aria-label={item.menuLabel}>
              {#each item.menu as entry (entry.id)}
                {@render control(entry)}
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {/each}
</div>

{#if wizard}
  <WikitextWizard
    {attachmentLookup}
    kind={wizard.kind}
    onCancel={() => closeWizard()}
    onInsert={(options) => closeWizard(options)}
    {pageLookup}
    source={wizard.source}
  />
{/if}

<style>
  .wikitext-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 18px;
    align-items: center;
    padding: 5px 6px;
    background: #f4f6f9;
    border: 1px solid #c9d0da;
    border-radius: 6px;
  }

  .toolbar-group {
    display: flex;
    gap: 1px;
    align-items: center;
  }

  .tool {
    display: inline-flex;
    gap: 0;
    align-items: center;
    justify-content: center;
    min-width: 30px;
    height: 30px;
    padding: 0 5px;
    font:
      600 14px/1 system-ui,
      -apple-system,
      "Segoe UI",
      sans-serif;
    color: #2b3441;
    cursor: pointer;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
  }

  .tool :global(svg) {
    flex: none;
  }

  .text :global(sup) {
    font-size: 9px;
    vertical-align: super;
  }

  .text :global(.mono) {
    font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
  }

  .tool:hover:not(:disabled) {
    background: #e2e8f0;
    border-color: #c9d0da;
  }

  .tool:active:not(:disabled) {
    background: #d5dde8;
  }

  .tool:focus-visible {
    outline: 2px solid #1f5fa8;
    outline-offset: 1px;
  }

  .tool:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .split {
    display: inline-flex;
    margin: 0 2px;
    border: 1px solid #d5dbe3;
    border-radius: 5px;
  }

  .split .tool {
    border-radius: 0;
  }

  .split .tool:first-child {
    border-radius: 4px 0 0 4px;
  }

  .tool.wizard {
    min-width: 22px;
    padding: 0 3px;
    color: #1f5fa8;
    border-left: 1px solid #d5dbe3;
    border-radius: 0 4px 4px 0;
  }

  .tool.wizard :global(svg) {
    width: 15px;
    height: 15px;
  }

  .tool.has-menu::after {
    margin-left: 3px;
    font-size: 8px;
    content: "▾";
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
    gap: 1px;
    padding: 4px;
    background: #fff;
    border: 1px solid #c9d0da;
    border-radius: 6px;
    box-shadow: 0 4px 12px rgb(0 0 0 / 12%);
  }

  .submenu-holder:hover .submenu,
  .submenu-holder:focus-within .submenu {
    display: flex;
  }
</style>
