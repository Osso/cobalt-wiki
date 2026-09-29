<script lang="ts">
  import { resolve } from "$app/paths"
  import { page } from "$app/state"
  import { ChevronDown } from "lucide"
  import LucideIcon from "$lib/component/LucideIcon.svelte"
  import UserAvatar from "$lib/component/UserAvatar.svelte"

  // Wikidot's header account links (#login-status), pointing at this site's pages.
  const user = $derived(page.data?.user_session?.user)
  const siteAdmin = $derived(page.data?.site_admin === true)
  const here = $derived(page.url.pathname + page.url.search)
  let optionsOpen = $state(false)
</script>

<div id="login-status">
  {#if user}
    <button
      id="account-topbutton"
      class:open={optionsOpen}
      aria-expanded={optionsOpen}
      onclick={() => (optionsOpen = !optionsOpen)}
      type="button"
    >
      <UserAvatar
        name={user.name}
        avatarHash={user.avatar_s3_hash}
        size={30}
        userId={user.user_id}
      />
      <span class="account-pill">
        <span class="printuser">{user.name}</span>
        <LucideIcon icon={ChevronDown} size="16px" />
      </span>
    </button>
    <div id="account-options" style:display={optionsOpen ? "block" : "none"}>
      <ul>
        <li><a id="my-account" href={resolve("/-/settings", {})}>My account</a></li>
        {#if siteAdmin}
          <li><a href={resolve("/-/admin/members", {})}>Site members</a></li>
        {/if}
        <li><a href={resolve("/-/logout", {})}>Sign out</a></li>
      </ul>
    </div>
  {:else}
    <a class="login-status-create-account btn" href={resolve("/-/register", {})}
      >Create account</a
    >
    <span>or</span>
    <a
      class="login-status-sign-in btn btn-primary"
      href={`${resolve("/-/login", {})}?origUrl=${encodeURIComponent(here)}`}>Sign in</a
    >
  {/if}
</div>
