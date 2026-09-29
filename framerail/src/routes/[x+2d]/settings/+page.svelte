<script lang="ts">
  import { enhance } from "$app/forms"
  import { resolve } from "$app/paths"
  import { onMount, untrack } from "svelte"
  import { squareAvatar, AVATAR_SIZE } from "$lib/avatar-resize"
  import UserAvatar from "$lib/component/UserAvatar.svelte"

  import type { PageProps } from "./$types"

  let { data, form }: PageProps = $props()

  let preview = $state<string | null>(null)
  let avatarError = $state("")

  async function chooseAvatar(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    if (preview) URL.revokeObjectURL(preview)
    preview = null
    avatarError = ""
    if (!file) return
    try {
      const resized = await squareAvatar(file)
      const files = new DataTransfer()
      files.items.add(resized)
      input.files = files.files
      preview = URL.createObjectURL(resized)
    } catch {
      input.value = ""
      avatarError = "That file could not be read as an image."
    }
  }

  // The submitted form's category stays open so its result is visible.
  const CATEGORY_OF: Record<string, string> = { avatar: "profile" }
  const categories = $derived([
    { id: "profile", label: "Profile" },
    { id: "email", label: "Email" },
    { id: "password", label: "Password" },
    ...(data.watchingPreferences ? [{ id: "watching", label: "Watching" }] : [])
  ])
  function submittedCategory(result: typeof form): string | null {
    const submitted = result?.section ?? (result?.message ? "watching" : null)
    return submitted ? (CATEGORY_OF[submitted] ?? submitted) : null
  }

  let active = $state(untrack(() => submittedCategory(form)) ?? "profile")

  $effect(() => {
    const submitted = submittedCategory(form)
    if (submitted) active = submitted
  })

  onMount(() => {
    const fromHash = location.hash.slice(1)
    if (categories.some((category) => category.id === fromHash)) active = fromHash
  })

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
  <div class="settings-layout">
    <nav class="settings-nav" aria-label="Account settings">
      {#each categories as category (category.id)}
        <a
          aria-current={active === category.id ? "page" : undefined}
          href="#{category.id}"
          onclick={() => (active = category.id)}>{category.label}</a
        >
      {/each}
    </nav>
    <div class="settings-panels">
      <section id="profile" aria-label="Profile" hidden={active !== "profile"}>
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

        <form
          class="settings-form"
          action="?/avatar"
          enctype="multipart/form-data"
          method="POST"
          use:enhance={() =>
            async ({ update }) => {
              await update()
              if (preview) URL.revokeObjectURL(preview)
              preview = null
            }}
        >
          <h2>Avatar</h2>
          {@render result("avatar")}
          {#if avatarError}<p class="settings-result error" role="alert">
              {avatarError}
            </p>{/if}
          <div class="avatar-row">
            {#if preview}
              <img class="avatar-preview" alt="New avatar preview" src={preview} />
            {:else}
              <UserAvatar
                name={data.account.name}
                avatarHash={data.account.avatarHash}
                size={72}
                userId={data.account.userId}
              />
            {/if}
            <div class="avatar-controls">
              <label>
                Image
                <input
                  name="avatar"
                  accept="image/*"
                  onchange={chooseAvatar}
                  type="file"
                />
              </label>
              <p class="avatar-hint">
                Cropped to a square and resized to {AVATAR_SIZE}×{AVATAR_SIZE} before upload.
              </p>
              <div class="avatar-buttons">
                <button class="btn btn-primary" disabled={!preview} type="submit">
                  Upload avatar
                </button>
                {#if data.account.avatarHash?.length}
                  <button name="remove" class="btn" type="submit" value="1"
                    >Remove avatar</button
                  >
                {/if}
              </div>
            </div>
          </div>
        </form>

        <form class="settings-form" action="?/profile" method="POST" use:enhance>
          <h2>Profile</h2>
          {@render result("profile")}
          <label>
            Real name
            <input
              name="realName"
              class="text"
              type="text"
              value={data.profile.realName}
            />
          </label>
          <label>
            Gender
            <input name="gender" class="text" type="text" value={data.profile.gender} />
          </label>
          <label>
            Birthday
            <input
              name="birthday"
              class="text"
              type="date"
              value={data.profile.birthday}
            />
          </label>
          <label>
            Location
            <input
              name="location"
              class="text"
              type="text"
              value={data.profile.location}
            />
          </label>
          <label>
            Website
            <input name="website" class="text" type="url" value={data.profile.website} />
          </label>
          <label>
            User page
            <input
              name="userPage"
              class="text"
              type="text"
              value={data.profile.userPage}
            />
          </label>
          <label>
            About you
            <textarea
              name="biography"
              class="text"
              rows="5"
              value={data.profile.biography}></textarea>
          </label>
          <button class="btn btn-primary" type="submit">Save profile</button>
        </form>
      </section>
      <section id="email" aria-label="Email" hidden={active !== "email"}>
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
      </section>
      <section id="password" aria-label="Password" hidden={active !== "password"}>
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
      </section>
      {#if data.watchingPreferences}
        <section id="watching" aria-label="Watching" hidden={active !== "watching"}>
          <form class="settings-form" action="?/watching" method="POST">
            <h2>Watching</h2>
            {@render result("watching")}
            <p>
              Activity stays on. Email and automatic watches are off until you opt in.
            </p>
            <label class="watching-choice">
              <input
                name="email_enabled"
                checked={data.watchingPreferences.email_enabled}
                type="checkbox"
              />
              Email me about watched pages
            </label>
            <label class="watching-choice">
              <input
                name="auto_watch"
                checked={data.watchingPreferences.auto_watch}
                type="checkbox"
              />
              Automatically watch pages after I edit them
            </label>
            <button class="btn btn-primary" type="submit"
              >Save watching preferences</button
            >
          </form>
          <h2>My subscriptions</h2>
          {#if form?.message && !form.section}<p role="status">{form.message}</p>{/if}
          {#if data.watchingSubscriptions?.length}
            <ul>
              {#each data.watchingSubscriptions as subscription (`${subscription.scope}:${subscription.target_id}`)}
                <li>
                  {subscription.scope} #{subscription.target_id}
                  <form action="?/unwatch" method="POST">
                    <input name="scope" type="hidden" value={subscription.scope} />
                    <input
                      name="target_id"
                      type="hidden"
                      value={subscription.target_id}
                    />
                    <input name="watching" type="hidden" value="false" />
                    <button class="btn btn-default" type="submit">Unwatch</button>
                  </form>
                </li>
              {/each}
            </ul>
          {:else}
            <p>No subscriptions on this site.</p>
          {/if}
          <p><a href={resolve("/-/activity", {})}>My Activity</a></p>
        </section>
      {/if}
    </div>
  </div>
{/if}

<style lang="scss">
  @use "../../../lib/css/account-form" as *;

  .settings-layout {
    display: grid;
    grid-template-columns: 11em minmax(0, 1fr);
    gap: 2em;
    align-items: start;
    margin-top: 1em;
  }

  .settings-nav {
    position: sticky;
    top: 1em;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px;
    background: #f4f6f9;
    border: 1px solid #d5dbe3;
    border-radius: 6px;

    a {
      padding: 0.45em 0.8em;
      color: #2b3441;
      text-decoration: none;
      border-radius: 4px;

      &:hover {
        background: #e2e8f0;
      }

      &[aria-current="page"] {
        font-weight: 600;
        color: #fff;
        background: #1f5fa8;
      }
    }
  }

  .settings-panels > section > :global(:first-child) {
    margin-top: 0;
  }

  @media (width <= 640px) {
    .settings-layout {
      grid-template-columns: minmax(0, 1fr);
      gap: 1em;
    }

    .settings-nav {
      position: static;
      flex-flow: row wrap;
    }
  }

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

  .avatar-row {
    display: flex;
    flex-wrap: wrap;
    gap: 1em;
    align-items: flex-start;
  }

  .avatar-preview {
    width: 72px;
    height: 72px;
    border-radius: 50%;
  }

  .avatar-controls {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 0.5em;
    min-width: 14em;
  }

  .avatar-hint {
    margin: 0;
    font-size: 0.85em;
    color: #555;
  }

  .avatar-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5em;
    button:disabled {
      cursor: not-allowed;
      opacity: 0.5;
    }
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
