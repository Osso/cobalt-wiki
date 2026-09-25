<script lang="ts">
  import SigmaEsque from "$lib/sigma-esque/sigma-esque.svelte"
  import Wikidot from "$lib/sigma-esque/wikidot.svelte"
  import wjBanner from "$assets/logo-outline.min.svg?raw"
  import ui from "$assets/ui.svg?raw"
  import ErrorPopup from "$lib/popup/error.svelte"
  import Toasts from "$lib/component/Toasts.svelte"
  import SearchBox from "$lib/component/SearchBox.svelte"
  import LoginStatus from "$lib/component/LoginStatus.svelte"

  import { page } from "$app/state"
  import { pageLayoutState, errorPopupState } from "$lib/stores.svelte"
  import { Layout } from "$lib/types"
  import { pageLayout } from "$lib/page-layout"
  import { asset, resolve } from "$app/paths"
  import { submitNewPage } from "$lib/new-page"
  import { clickSiteChanges } from "$lib/site-changes"
  import { clickCollapsible } from "$lib/collapsible"
  import { clickTabview, keydownTabview } from "$lib/tabview"

  let { children } = $props()

  function closeErrorPopup() {
    errorPopupState.current = {
      state: false,
      message: null,
      data: null
    }
  }

  // Derived, so server rendering already uses the site's layout; the store
  // mirrors it for components such as the error popup.
  const layout = $derived(pageLayout(page))
  $effect(() => {
    pageLayoutState.current = layout
  })

  // Capture phase: SvelteKit's router takes same-origin GET form submits
  // (NewPage's action is "dummy.html") unless an earlier listener prevents them.
  $effect(() => {
    const onSubmit = (event: SubmitEvent) =>
      submitNewPage(
        event,
        (path) => window.location.assign(path),
        (message) => {
          errorPopupState.current = { state: true, message, data: null }
        }
      )
    const onClick = (event: MouseEvent) => {
      clickSiteChanges(event, (path) => window.location.assign(path))
      clickCollapsible(event)
      clickTabview(event)
    }
    window.addEventListener("submit", onSubmit, true)
    window.addEventListener("click", onClick, true)
    window.addEventListener("keydown", keydownTabview, true)
    return () => {
      window.removeEventListener("submit", onSubmit, true)
      window.removeEventListener("click", onClick, true)
      window.removeEventListener("keydown", keydownTabview, true)
    }
  })
</script>

<div class="svg-defs hidden">
  {@html ui}
</div>

{#if errorPopupState.current.state}
  <ErrorPopup exitPrompt={closeErrorPopup} />
{/if}

<div id="toasts">
  <Toasts />
</div>

<svelte:head>
  <title>{page.data.site?.name}</title>
  <!-- One icon per site, rendered on the server so no other icon shows first. -->
  {#if page.data.site?.slug === "cobalt-company"}
    <!-- Cobalt's Wikidot favicon (local--favicon/favicon.gif, a PNG) -->
    <link href="/cobalt-favicon.png" rel="icon" type="image/png" />
  {:else}
    <link href={asset("/favicon.png")} rel="icon" />
  {/if}
</svelte:head>

{#if layout === Layout.WIKIDOT}
  <link
    href="https://d3g0gp89917ko0.cloudfront.net/v--7690939296dc/common--theme/base/css/style.css"
    rel="stylesheet"
  />
  <link
    href="https://d3g0gp89917ko0.cloudfront.net/v--7690939296dc/common--modules/css/pagerate/PageRateWidgetModule.css"
    rel="stylesheet"
  />
  <link
    href={page.data.site?.slug === "cobalt-company"
      ? "/-/cobalt-theme.css"
      : "https://cdn.scpwiki.com/theme/en/sigma/css/sigma.min.css"}
    rel="stylesheet"
  />
  <Wikidot
    sideBarHtml={page.data?.compiled_side_bar_html ?? page.error?.compiled_side_bar_html}
  >
    {#snippet header()}
      <h1>
        <a class="active" href={resolve("/", {})}><span>{page.data.site?.name}</span></a>
      </h1>
      <h2>
        <span>{page.data.site?.tagline}</span>
      </h2>
      <SearchBox />
      <LoginStatus />
    {/snippet}

    {#snippet topBar()}
      {@html page.data?.compiled_top_bar_html ?? page.error?.compiled_top_bar_html ?? ""}
    {/snippet}

    {#snippet content()}
      {@render children?.()}
    {/snippet}

    {#snippet footer()}
      <div class="options">
        <a href={resolve("/", {})}
          >{page.data?.internationalization?.docs ??
            page.error?.internationalization?.docs}</a
        >
        |
        <a href={resolve("/", {})}
          >{page.data?.internationalization?.["terms-conditions"] ??
            page.error?.internationalization?.["terms-conditions"]}</a
        >
        |
        <a href={resolve("/", {})}
          >{page.data?.internationalization?.privacy ??
            page.error?.internationalization?.privacy}</a
        >
        |
        <a href={resolve("/", {})}
          >{page.data?.internationalization?.security ??
            page.error?.internationalization?.security}</a
        >
      </div>
      <div class="footer-powered-by">
        {page.data?.internationalization?.["footer-powered-by"] ??
          page.error?.internationalization?.["footer-powered-by"]}
      </div>
    {/snippet}
    {#snippet license()}
      {@html page.data?.internationalization?.["footer-license-unless"] ??
        page.error?.internationalization?.["footer-license-unless"]}
    {/snippet}
  </Wikidot>
{:else}
  <SigmaEsque>
    {#snippet header()}
      <div class="header-wjbanner">
        {@html wjBanner}
      </div>
    {/snippet}

    {#snippet topBar()}
      {@html page.data?.compiled_top_bar_html ?? page.error?.compiled_top_bar_html ?? ""}
    {/snippet}

    {#snippet content()}
      {@render children?.()}
    {/snippet}

    {#snippet footer()}
      <div class="footer-inner">
        <ul class="footer-items">
          <li class="footer-item">
            <a href={resolve("/", {})}
              >{page.data?.internationalization?.["terms-conditions"] ??
                page.error?.internationalization?.["terms-conditions"]}</a
            >
          </li>
          <li class="footer-item">
            <a href={resolve("/", {})}
              >{page.data?.internationalization?.privacy ??
                page.error?.internationalization?.privacy}</a
            >
          </li>
          <li class="footer-item">
            <a href={resolve("/", {})}
              >{page.data?.internationalization?.docs ??
                page.error?.internationalization?.docs}</a
            >
          </li>
          <li class="footer-item">
            <a href={resolve("/", {})}
              >{page.data?.internationalization?.security ??
                page.error?.internationalization?.security}</a
            >
          </li>
        </ul>
        <div class="footer-powered-by">
          {page.data?.internationalization?.["footer-powered-by"] ??
            page.error?.internationalization?.["footer-powered-by"]}
        </div>
      </div>
    {/snippet}
  </SigmaEsque>
{/if}

<!-- Ignoring the "unused" svg as we know we imported and embedded a raw svg -->
<!-- svelte-ignore css_unused_selector -->
<style global lang="scss">
  @use "../lib/css/abstracts/variables" as *;

  $tablet-max-width: 767px;

  .header-wjbanner {
    height: 80%;
    color: #fff;

    svg {
      width: auto;
      height: 100%;
    }
  }

  .footer-inner {
    display: flex;
    flex-direction: row;
    gap: 10px;
    align-items: center;
    justify-content: stretch;
    width: 100%;
  }

  .footer-items {
    display: flex;
    flex: 1;
    flex-direction: row;
    gap: 10px;
    align-items: center;
    justify-content: flex-start;
    padding: 0;
    list-style: none;

    .footer-item a {
      color: #fff;
      text-decoration: none;
    }
  }

  #toasts,
  #modals {
    position: fixed;
    bottom: 0;
    left: 0;
    z-index: $z-dialog;
    width: 100%;
  }

  @media (max-width: $tablet-max-width) {
    .header-wjbanner {
      text-align: center;

      svg {
        height: initial;
        max-height: 6.5em;
      }
    }
  }
</style>
