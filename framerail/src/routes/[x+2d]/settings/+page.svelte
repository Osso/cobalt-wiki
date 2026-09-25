<script lang="ts">
  import { enhance } from "$app/forms"
  import { resolve } from "$app/paths"

  import type { PageProps } from "./$types"

  let { data, form }: PageProps = $props()

  const loginUrl = `${resolve("/-/login", {})}?origUrl=${encodeURIComponent("/-/settings")}`
</script>

{#snippet result(section: string)}
  {#if form?.section === section}
    <p class={["settings-result", form.saved ? "saved" : "error"]} role="status">
      {form.message}
    </p>
  {/if}
{/snippet}

<h1>My account</h1>

{#if !data.account || !data.profile}
  <p>You need to <a href={loginUrl}>sign in</a> to change your account settings.</p>
{:else}
  <dl class="settings-account">
    <dt>Name</dt>
    <dd>{data.account.name}</dd>
    <dt>Profile</dt>
    <dd>
      <a href={resolve("/-/user/[slug]", { slug: data.account.slug })}
        >{data.account.slug}</a
      >
    </dd>
    <dt>Email</dt>
    <dd>{data.account.email || "None"}</dd>
  </dl>

  {#if data.watchingPreferences}
    <form class="settings-form" action="?/watching" method="POST">
      <h2>Watching</h2>
      {@render result("watching")}
      <p>Activity stays on. Email and automatic watches are off until you opt in.</p>
      <label class="watching-choice">
        <input
          type="checkbox"
          name="email_enabled"
          checked={data.watchingPreferences.email_enabled}
        />
        Email me about watched pages
      </label>
      <label class="watching-choice">
        <input
          type="checkbox"
          name="auto_watch"
          checked={data.watchingPreferences.auto_watch}
        />
        Automatically watch pages after I edit them
      </label>
      <button class="btn btn-primary" type="submit">Save watching preferences</button>
    </form>
    <h2>My subscriptions</h2>
    {#if form?.message && !form.section}<p role="status">{form.message}</p>{/if}
    {#if data.watchingSubscriptions?.length}
      <ul>
        {#each data.watchingSubscriptions as subscription}
          <li>
            {subscription.scope} #{subscription.target_id}
            <form action="?/unwatch" method="POST">
              <input type="hidden" name="scope" value={subscription.scope} />
              <input type="hidden" name="target_id" value={subscription.target_id} />
              <input type="hidden" name="watching" value="false" />
              <button class="btn btn-default" type="submit">Unwatch</button>
            </form>
          </li>
        {/each}
      </ul>
    {:else}
      <p>No subscriptions on this site.</p>
    {/if}
    <p><a href={resolve("/-/activity", {})}>My Activity</a></p>
  {/if}

  <form class="settings-form" action="?/profile" method="POST" use:enhance>
    <h2>Profile</h2>
    {@render result("profile")}
    <label>
      Real name
      <input name="realName" class="text" type="text" value={data.profile.realName} />
    </label>
    <label>
      Gender
      <input name="gender" class="text" type="text" value={data.profile.gender} />
    </label>
    <label>
      Birthday
      <input name="birthday" class="text" type="date" value={data.profile.birthday} />
    </label>
    <label>
      Location
      <input name="location" class="text" type="text" value={data.profile.location} />
    </label>
    <label>
      Website
      <input name="website" class="text" type="url" value={data.profile.website} />
    </label>
    <label>
      User page
      <input name="userPage" class="text" type="text" value={data.profile.userPage} />
    </label>
    <label>
      About you
      <textarea name="biography" class="text" rows="5" value={data.profile.biography}
      ></textarea>
    </label>
    <button class="btn btn-primary" type="submit">Save profile</button>
  </form>

  <form class="settings-form" action="?/email" method="POST" use:enhance>
    <h2>Email address</h2>
    {@render result("email")}
    <label>
      New email address
      <input name="email" class="text" autocomplete="email" required type="email" />
    </label>
    <label>
      Current password
      <input
        name="currentPassword"
        class="text"
        autocomplete="current-password"
        required
        type="password"
      />
    </label>
    <button class="btn btn-primary" type="submit">Change email</button>
  </form>

  <form class="settings-form" action="?/password" method="POST" use:enhance>
    <h2>Password</h2>
    {@render result("password")}
    <label>
      Current password
      <input
        name="currentPassword"
        class="text"
        autocomplete="current-password"
        required
        type="password"
      />
    </label>
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
    <button class="btn btn-primary" type="submit">Change password</button>
  </form>
{/if}

<style lang="scss">
  @use "../../../lib/css/account-form" as *;

  .settings-account {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.3em 1em;
    max-width: 32em;

    dt {
      font-weight: bold;
    }

    dd {
      margin: 0;
      overflow-wrap: anywhere;
    }
  }

  .settings-form {
    display: flex;
    flex-direction: column;
    gap: 0.75em;
    max-width: 32em;
    margin: 1.5em 0;

    label {
      display: flex;
      flex-direction: column;
      gap: 0.25em;
    }

    .watching-choice {
      flex-direction: row;
      align-items: center;
    }

    @include account-form-controls(".text");
  }

  .settings-result {
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
