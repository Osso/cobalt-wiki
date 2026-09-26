<script lang="ts">
  import type { PageProps } from "./$types"

  let { data }: PageProps = $props()
</script>

<svelte:head>
  <title>{data.page_revision.title} | {data.site.name}</title>
</svelte:head>

<main class="print-page">
  <div class="print-controls">
    <button onclick={() => window.print()} type="button">Print the page</button>
  </div>
  <p class="source">Source page: <a href={data.sourceUrl}>{data.sourceUrl}</a></p>
  {#if data.page_revision.title}
    <h1>{data.page_revision.title}</h1>
  {/if}
  <div class="print-body">{@html data.compiled_body_html}</div>
  {#if data.internationalization?.["footer-license-unless"]}
    <footer class="license-area">
      {@html data.internationalization["footer-license-unless"]}
    </footer>
  {/if}
</main>

<style>
  .print-page {
    box-sizing: border-box;
    max-width: 70ch;
    padding: 0 1rem;
    margin: 2rem auto;
    font:
      1rem/1.6 Georgia,
      serif;
    color: #111;
    overflow-wrap: anywhere;
  }

  .print-controls {
    text-align: right;
  }

  button {
    min-height: 44px;
    padding: 0.5rem 1rem;
    cursor: pointer;
  }

  button:focus-visible,
  a:focus-visible {
    outline: 3px solid currentColor;
    outline-offset: 3px;
  }

  .source {
    font-size: 0.875rem;
  }

  h1 {
    line-height: 1.2;
  }

  .license-area {
    padding-top: 1rem;
    margin-top: 2rem;
    border-top: 1px solid currentColor;
  }

  @media print {
    .print-page {
      max-width: none;
      padding: 0;
      margin: 0;
    }

    .print-controls {
      display: none;
    }
  }
</style>
