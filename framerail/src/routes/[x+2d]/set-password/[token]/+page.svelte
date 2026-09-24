<script lang="ts">
  import { enhance } from "$app/forms"
  import { resolve } from "$app/paths"

  import type { PageProps } from "./$types"

  let { form }: PageProps = $props()
</script>

<h1>Choose your password</h1>

{#if form && "saved" in form}
  <p class="password-result saved" role="status">Your password is set.</p>
  <p><a href={resolve("/-/login", {})}>Sign in</a> with your name and new password.</p>
{:else}
  <form class="password-form" method="POST" use:enhance>
    {#if form?.message}
      <p class="password-result error" role="status">{form.message}</p>
    {/if}
    {#if form && "linkUnusable" in form}
      <p><a href={resolve("/-/forgot-password", {})}>Get a new link</a></p>
    {/if}
    <label>
      New password
      <input
        name="newPassword"
        class="text"
        autocomplete="new-password"
        required
        type="password"
      />
    </label>
    <label>
      Repeat new password
      <input
        name="confirmPassword"
        class="text"
        autocomplete="new-password"
        required
        type="password"
      />
    </label>
    <button class="btn btn-primary" type="submit">Set password</button>
  </form>
{/if}

<style lang="scss">
  @use "../../../../lib/css/account-form" as *;

  .password-form {
    display: flex;
    flex-direction: column;
    gap: 0.75em;
    max-width: 24em;
    margin: 1em 0;

    label {
      display: flex;
      flex-direction: column;
      gap: 0.25em;
    }

    @include account-form-controls;
  }

  .password-result {
    padding: 0.5em 0.75em;
    margin: 0;
    border: 1px solid;

    &.saved {
      color: #1d5b1d;
      background: #eef8ee;
    }

    &.error {
      color: #8a1f1f;
      background: #fbeeee;
    }
  }
</style>
