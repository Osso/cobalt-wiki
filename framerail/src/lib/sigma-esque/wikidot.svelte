<script lang="ts">
  let { header, topBar, sideBarHtml, content, footer, license } = $props()
</script>

<div id="skrollr-body">
  <!-- svelte-ignore a11y_consider_explicit_label -->
  <a id="page-top"></a>
  <div id="container-wrap-wrap">
    <div id="container-wrap">
      <div id="container">
        <div id="header">
          {@render header?.()}
          <div id="top-bar">
            {@render topBar?.()}
          </div>
        </div>
        <div id="content-wrap">
          {#if sideBarHtml}
            <div id="side-bar">
              {@html sideBarHtml}
            </div>
          {/if}
          <div id="main-content">
            <div id="action-area-top"></div>
            {@render content?.()}
          </div>
        </div>
        <div id="footer">
          {@render footer?.()}
        </div>
        <div id="license-area" class="license-area">
          {@render license?.()}
        </div>
        <div id="extrac-div-1"><span></span></div>
        <div id="extrac-div-2"><span></span></div>
        <div id="extrac-div-3"><span></span></div>
      </div>
      <div id="extra-div-1"><span></span></div>
      <div id="extra-div-2"><span></span></div>
      <div id="extra-div-3"><span></span></div>
      <div id="extra-div-4"><span></span></div>
      <div id="extra-div-5"><span></span></div>
      <div id="extra-div-6"><span></span></div>
    </div>
  </div>
</div>

<!-- Ignoring the styling as being a theme it will inevitably style other elements in the entire layout -->
<!-- svelte-ignore css_unused_selector -->
<style global lang="scss">
  $tablet-max-width: 767px;

  :root {
    /* Fallback colors */
    --text: #111;
    --background: #fff;
    --border: #0006;
    --mild-text: #f0f0f0;
    --accent: #0066cc;
    --error: #b42d0a;
    --col-accent-1: #19a9ff;
    --col-accent-2: #0068b5;
  }

  .clickable {
    cursor: pointer;
    user-select: none;

    &:disabled {
      cursor: auto;
    }
  }

  .hidden {
    display: none;
  }

  .wj-monospace {
    font-size: 0.98em;
  }

  @media (prefers-color-scheme: light) {
    :root {
      /* Fallback colors */
      --text: #111;
      --background: #fff;
      --border: #0006;
      --mild-text: #f0f0f0;
      --accent: #0066cc;
    }
  }

  @media (prefers-color-scheme: dark) {
    :root {
      /* Fallback colors */
      --text: #b6c2cf;
      --background: #222;
      --border: #b6c2cf;
      --mild-text: #bbb;
      --accent: #44aaff;
    }
  }

  /*
   * Replica chrome over the site theme's dark background image: the account
   * links, search box and footer sit on translucent dark panels so their
   * text stays readable (Wikidot's own styling is not required here).
   */
  $chrome-panel: rgb(8 12 36 / 72%);
  $chrome-text: #e6eaff;
  $chrome-link: #9fd4ff;

  #header #login-status {
    padding: 0.25em 0.75em;
    color: $chrome-text;
    background: $chrome-panel;
    border-radius: 6px;

    a,
    a:visited {
      color: $chrome-link;
      text-decoration: none;
      background: none;
      border: 0;
      box-shadow: none;
    }

    a:hover {
      text-decoration: underline;
    }

    .printuser {
      font-weight: bold;
      color: $chrome-text;
    }

    #account-topbutton {
      padding: 0 0.25em;
      font-size: 0.8em;
    }

    #account-options {
      position: absolute;
      right: 0;
      z-index: 20;
      margin-top: 0.25em;
      background: $chrome-panel;
      border-radius: 6px;

      ul {
        padding: 0.4em 0.9em;
        margin: 0;
        list-style: none;
      }
    }
  }

  #header #search-top-box {
    form {
      display: flex;
      gap: 0.35em;
      align-items: stretch;
      padding: 0.3em;
      background: $chrome-panel;
      border-radius: 6px;
    }

    input#search-top-box-input {
      box-sizing: border-box;
      height: 2em;
      padding: 0 0.6em;
      font-size: 0.95em;
      color: #111;
      background: #fff;
      border: 1px solid #7d8fc4;
      border-radius: 4px;
    }

    input.button {
      height: 2em;
      padding: 0 0.9em;
      font-size: 0.95em;
      color: #fff;
      cursor: pointer;
      background: #1f5fa8;
      border: 1px solid #3f7fcf;
      border-radius: 4px;
    }

    input.button:hover {
      background: #2a72c4;
    }
  }

  #container #footer,
  #container #license-area {
    box-sizing: border-box;
    color: $chrome-text;
    background: $chrome-panel;

    a,
    a:visited {
      color: $chrome-link;
    }
  }

  #container #footer {
    padding: 0.5em 1em;
    border-radius: 6px 6px 0 0;
  }

  #container #license-area {
    padding: 0.4em 1em 0.6em;
    border-radius: 0 0 6px 6px;
  }

  // Map Wikidot's base YUI tab skin to FTML's compiled tab markup.
  // Source: common--theme/base/css/style.css (Yahoo! 2007, BSD licensed).
  .wj-tabs {
    display: block;
  }

  .wj-tabs > .wj-tabs-button-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0 0.16em;
    align-items: flex-end;
    border-bottom: 5px solid #2647a0;
  }

  .wj-tabs-button-list > .wj-tabs-button {
    box-sizing: border-box;
    display: block;
    max-width: 100%;
    padding: 0.25em 0.75em;
    color: #000;
    cursor: pointer;
    background: #d8d8d8
      url("https://d3g0gp89917ko0.cloudfront.net/v--7690939296dc/common--javascript/yahooui/assets/sprite.png")
      repeat-x;
    border: 1px solid #a3a3a3;

    &:hover,
    &:focus-visible {
      background-color: #bfdaff;
      background-position: left -1300px;
    }

    &[aria-selected="true"] {
      padding: 0.35em 0.75em;
      color: #fff;
      background-color: #2647a0;
      background-position: left -1400px;
      border-color: #243356;
    }
  }

  .wj-tabs > .wj-tabs-panel-list {
    padding: 0.25em 0.5em;
    background: #f4f5ff;
    border: 1px solid #808080;
    border-top-color: #243356;
  }

  @media (max-width: $tablet-max-width) {
    .header {
      height: initial;
    }
  }
</style>
