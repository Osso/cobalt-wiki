<script lang="ts">
  import { enhance } from "$app/forms"
  import { resolve } from "$app/paths"
  import type { PageProps } from "./$types"

  let { data, form }: PageProps = $props()
  let submitting = $state(false)
  const loginUrl = `${resolve("/-/login", {})}?origUrl=${encodeURIComponent("/-/join")}`
</script>

<h1>Apply to become a member</h1>

{#if !data.signedIn}
  <p>
    <a href={loginUrl}>Sign in</a> to apply.
    <a href={resolve("/-/register", {})}>Create an account</a> if you do not have one. New accounts
    are guests; membership requires administrator approval.
  </p>
{:else if data.status?.is_member}
  <p>You are already a member of this site.</p>
{:else if data.status?.application || form?.submitted}
  <p role="status">
    Your application is awaiting administrator review. You remain a guest until approved.
  </p>
  {#if data.status?.application}
    <p class="application-message">{data.status.application.message}</p>
  {/if}
{:else}
  <p>
    Tell the administrators why you would like to join. Applying does not grant editing
    access.
  </p>
  <form
    method="POST"
    use:enhance={() => {
      submitting = true
      return async ({ update }) => {
        try {
          await update()
        } finally {
          submitting = false
        }
      }
    }}
  >
    <label for="application-message">Why would you like to join?</label>
    <textarea
      id="application-message"
      name="message"
      rows="6"
      required
      aria-describedby="application-limit"
      value={form?.applicationMessage ?? ""}></textarea>
    <p id="application-limit">Maximum 2,000 characters.</p>
    {#if form?.message}
      <p class="error" role="alert">{form.message}</p>
    {/if}
    <button type="submit" disabled={submitting}
      >{submitting ? "Submitting…" : "Submit application"}</button
    >
  </form>
{/if}

<style lang="scss">
  @use "../../../lib/css/account-form" as *;

  form {
    display: flex;
    flex-direction: column;
    gap: 0.5em;
    max-width: 40em;
    @include account-form-controls;
  }

  textarea {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
  }

  button {
    align-self: flex-start;
    min-height: 44px;
  }

  .application-message {
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }

  .error {
    color: #a12a2a;
  }
</style>
