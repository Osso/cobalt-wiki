<script lang="ts">
  import { goto, invalidateAll } from "$app/navigation"
  import { resolve } from "$app/paths"
  import { page } from "$app/state"
  import { errorPopupState } from "$lib/stores.svelte"
  import { superForm } from "sveltekit-superforms"
  import { untrack } from "svelte"
  import { toast } from "$lib/component/scripts/toasts"
  import { ToastType } from "$lib/types"

  import type { PageProps } from "./$types"

  let { data }: PageProps = $props()

  let isLoggedIn = $derived<boolean>(data.isLoggedIn)

  // The header's Sign in link passes the page to return to, as Wikidot does.
  const returnPath = $derived.by(() => {
    const origUrl = page.url.searchParams.get("origUrl") ?? "/"
    return origUrl.startsWith("/") && !origUrl.startsWith("//") ? origUrl : "/"
  })

  const { form, enhance } = superForm(
    untrack(() => data.loginForm),
    {
      onResult: async ({ result }) => {
        if (result.type === "success" && result.data) {
          toast(ToastType.Success, data.internationalization!["login.toast"]!)
          isLoggedIn = true
          await invalidateAll()
          await goto(returnPath)
          return
        }

        if (result.type === "failure" && result.data) {
          errorPopupState.current = {
            state: true,
            message: result.data?.message,
            data: result.data?.data
          }
        }
      }
    }
  )
</script>

<h1 id="login-title">Sign in to {page.data.site?.name}</h1>
{#if isLoggedIn}
  <p>{data.internationalization?.["login.toast"]}</p>
{:else}
  <form id="login" class="login-form" method="POST" use:enhance>
    <input
      name="nameOrEmail"
      class="text"
      autocomplete="username"
      placeholder="username or email address"
      type="text"
      bind:value={$form.nameOrEmail}
    />
    <input
      name="password"
      class="text"
      autocomplete="current-password"
      placeholder="password"
      type="password"
      bind:value={$form.password}
    />
    <button class="btn btn-primary" type="submit">Sign in</button>
    <p>
      No account yet? <a href={resolve("/-/register", {})}>Create account</a>
    </p>
  </form>
{/if}

<style lang="scss">
  .login-form {
    display: flex;
    flex-direction: column;
    gap: 0.75em;
    max-width: 24em;
    margin: 1em auto;

    input {
      padding: 0.4em;
      font-size: 1.1em;
    }

    button {
      align-self: flex-start;
    }
  }

  #login-title {
    text-align: center;
  }
</style>
