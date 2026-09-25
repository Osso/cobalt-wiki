<script lang="ts">
  import { onMount, untrack } from "svelte"

  let {
    images,
    initialIndex,
    onclose
  }: {
    images: { src: string; alt: string }[]
    initialIndex: number
    onclose: () => void
  } = $props()

  let dialog: HTMLDialogElement
  let selectedIndex = $state(untrack(() => initialIndex))
  let imageError = $state(false)
  const image = $derived(images[selectedIndex])

  onMount(() => {
    dialog.showModal()
  })

  function moveBy(offset: number) {
    selectedIndex += offset
    imageError = false
  }
</script>

<dialog bind:this={dialog} aria-label="Gallery viewer" {onclose}>
  <div class="viewer">
    {#if imageError}
      <p role="alert">Unable to load image.</p>
    {:else}
      <img src={image.src} alt={image.alt} onerror={() => (imageError = true)} />
    {/if}
    <div class="controls">
      <span aria-live="polite">Image {selectedIndex + 1} of {images.length}</span>
      <button type="button" disabled={selectedIndex === 0} onclick={() => moveBy(-1)}
        >Previous</button
      >
      <button
        type="button"
        disabled={selectedIndex === images.length - 1}
        onclick={() => moveBy(1)}>Next</button
      >
      <button type="button" onclick={() => dialog.close()}>Close</button>
    </div>
  </div>
</dialog>

<style>
  dialog {
    max-width: calc(100vw - 32px);
    max-height: calc(100dvh - 32px);
    padding: 12px;
    color: #111;
    background: #fff;
    border: 0;
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 80%);
  }

  .viewer {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  img {
    display: block;
    max-width: 100%;
    max-height: calc(100dvh - 120px);
    width: auto;
    height: auto;
    object-fit: contain;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }

  button {
    min-height: 44px;
    padding: 8px 12px;
    color: #111;
    background: #fff;
    border: 1px solid #777;
    cursor: pointer;
  }

  button:focus-visible {
    outline: 2px solid #111;
    outline-offset: 2px;
  }

  button:disabled {
    color: #666;
    cursor: default;
  }
</style>
