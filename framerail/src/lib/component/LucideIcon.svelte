<!--
  @component Inline SVG for a `lucide` icon node, drawn in the current text color.
-->
<script lang="ts">
  import type { IconNode } from "lucide"

  let { icon, size = "1em" }: { icon: IconNode; size?: string } = $props()
</script>

<svg
  aria-hidden="true"
  fill="none"
  height={size}
  stroke="currentColor"
  stroke-linecap="round"
  stroke-linejoin="round"
  stroke-width="2"
  viewBox="0 0 24 24"
  width={size}
>
  {#each icon as [tag, attributes], index (index)}
    <svelte:element this={tag} {...attributes} />
  {/each}
</svg>
