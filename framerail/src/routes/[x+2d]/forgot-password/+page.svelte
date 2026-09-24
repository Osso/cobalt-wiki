<script lang="ts">
  import { enhance } from "$app/forms"

  import type { PageProps } from "./$types"

  let { form }: PageProps = $props()
</script>

<h1>Forgotten your password?</h1>

{#if form && "sent" in form}
  <p class="password-result sent" role="status">
    If an account uses that address, we have emailed it a link to choose a new password.
    The link works once, for 24 hours.
  </p>
{:else}
  <form class="password-form" method="POST" use:enhance>
    <p>
      Enter the email address of your account and we will send you a link to choose a new
      password.
    </p>
    {#if form?.message}
      <p class="password-result error" role="status">{form.message}</p>
    {/if}
    <label>
      Email address
      <input name="email" class="text" autocomplete="email" required type="email" />
    </label>
    <button class="btn btn-primary" type="submit">Send link</button>
  </form>
{/if}

<style lang="scss">
  @use "../../../lib/css/account-form" as *;

  .password-form {
    display: flex;
    flex-direction: column;
    gap: 0.75em;
    max-width: 24em;
    margin: 1em 0;

    p {
      margin: 0;
    }

    label {
      display: flex;
      flex-direction: column;
      gap: 0.25em;
    }

    @include account-form-controls;
  }

  .password-result {
    padding: 0.5em 0.75em;
    margin: 1em 0;
    border: 1px solid;

    &.sent {
      color: #1d5b1d;
      background: #eef8ee;
    }

    &.error {
      margin: 0;
      color: #8a1f1f;
      background: #fbeeee;
    }
  }
</style>
