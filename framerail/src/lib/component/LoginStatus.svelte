<script lang="ts">
  import { page } from "$app/state"

  // Wikidot's header account links (#login-status), pointing at this site's pages.
  const user = $derived(page.data?.user_session?.user)
  const here = $derived(page.url.pathname + page.url.search)
  let optionsOpen = $state(false)
</script>

<div id="login-status">
  {#if user}
    <span class="printuser">{user.name}</span> |
    <a id="my-account" href="/-/settings">My account</a>
    <!-- svelte-ignore a11y_invalid_attribute -->
    <a
      id="account-topbutton"
      href="javascript:;"
      aria-label="Account options"
      onclick={() => (optionsOpen = !optionsOpen)}>&#9660;</a
    >
    <div id="account-options" style:display={optionsOpen ? "block" : "none"}>
      <ul>
        <li><a href="/-/settings">My account</a></li>
        <li><a href="/-/logout">Sign out</a></li>
      </ul>
    </div>
  {:else}
    <a class="login-status-create-account btn" href="/-/register">Create account</a>
    <span>or</span>
    <a
      class="login-status-sign-in btn btn-primary"
      href={`/-/login?origUrl=${encodeURIComponent(here)}`}>Sign in</a
    >
  {/if}
</div>
