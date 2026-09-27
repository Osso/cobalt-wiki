<script lang="ts">
  import { resolve } from "$app/paths"
  import type { userEditSchema } from "$lib/server/load/user"
  import type { InferOutput } from "valibot"
  import type { PageData } from "./$types"

  let {
    data,
    userData
  }: { data: PageData; userData: InferOutput<typeof userEditSchema> } = $props()

  type TranslationKey = keyof NonNullable<PageData["internationalization"]>
  const label = (key: TranslationKey, fallback: string) =>
    (data.internationalization?.[key] ?? fallback).replace(/:\s*$/, "")

  // svelte-ignore state_referenced_locally
  let avatar = $state(
    data.user?.avatar_s3_hash
      ? `https://${data.site_file_domain}/-/avatar/${data.user.user_id}`
      : null
  )

  $effect(() => {
    let url: string | undefined
    // If the user has edited their avatar, use the new avatar
    if (userData?.avatar) {
      url = URL.createObjectURL(userData.avatar)
      avatar = url
    }

    return () => {
      if (url) {
        URL.revokeObjectURL(url)
      }
    }
  })

  let displayName = $derived(userData?.name || data.user?.name || data.user?.slug || "")
  let isOwnProfile = $derived(
    Boolean(data.user) && data.user_session?.user?.user_id === data.user?.user_id
  )
  let memberSince = $derived(
    data.user?.created_at
      ? new Intl.DateTimeFormat("en-GB", {
          year: "numeric",
          month: "long",
          day: "numeric",
          timeZone: "UTC"
        }).format(new Date(data.user.created_at))
      : null
  )

  /** Only http(s) URLs become links; anything else is shown as text. */
  function webUrl(value: string | null | undefined): string | null {
    if (!value) return null
    try {
      const url = new URL(value)
      return url.protocol === "http:" || url.protocol === "https:" ? url.href : null
    } catch {
      return null
    }
  }

  let details = $derived(
    [
      {
        key: "real-name",
        label: label("user-profile-info.real-name", "Real name"),
        value: userData?.realName
      },
      {
        key: "gender",
        label: label("user-profile-info.gender", "Gender"),
        value: userData?.gender
      },
      {
        key: "birthday",
        label: label("user-profile-info.birthday", "Birthday"),
        value: userData?.birthday
      },
      {
        key: "location",
        label: label("user-profile-info.location", "Location"),
        value: userData?.location
      },
      {
        key: "locales",
        label: label("user-profile-info.locales", "Languages"),
        value: userData?.locales?.split(" ").join(", ")
      }
    ].filter((item) => item.value)
  )
  let links = $derived(
    [
      {
        key: "website",
        label: label("user-profile-info.website", "Website"),
        value: userData?.website
      },
      {
        key: "user-page",
        label: label("user-profile-info.user-page", "User page"),
        value: userData?.userPage
      }
    ].filter((item) => item.value)
  )
</script>

<svelte:head>
  <title>{displayName} | {data?.site?.name}</title>
</svelte:head>

<article class="user-profile" data-id={data.user?.user_id}>
  <header class="profile-header">
    {#if avatar}
      <img class="profile-avatar" alt={label("avatar", "Avatar")} src={avatar} />
    {:else}
      <span class="profile-avatar is-initial" aria-hidden="true"
        >{displayName.charAt(0).toUpperCase()}</span
      >
    {/if}
    <div class="profile-identity">
      <h1 class="user-attribute name">{displayName}</h1>
      <p class="profile-meta">
        <span class="profile-slug">{data.user?.slug}</span>
        {#if memberSince}
          <span aria-hidden="true">·</span>
          <span class="profile-since"
            >Account created <time datetime={data.user?.created_at}>{memberSince}</time
            ></span
          >
        {/if}
      </p>
    </div>
    {#if isOwnProfile}
      <a class="profile-edit" href={resolve("/-/settings", {})}>Edit profile</a>
    {/if}
  </header>

  {#if details.length || links.length}
    <dl class="profile-details">
      {#each details as item (item.key)}
        <div class="user-attribute {item.key}">
          <dt>{item.label}</dt>
          <dd>{item.value}</dd>
        </div>
      {/each}
      {#each links as item (item.key)}
        {@const href = webUrl(item.value)}
        <div class="user-attribute {item.key}">
          <dt>{item.label}</dt>
          <dd>
            {#if href}<a {href} rel="external nofollow noopener">{item.value}</a
              >{:else}{item.value}{/if}
          </dd>
        </div>
      {/each}
    </dl>
  {/if}

  {#if userData?.biography}
    <section class="user-attribute biography">
      <h2>{label("user-profile-info.biography", "About")}</h2>
      <p>{userData.biography}</p>
    </section>
  {/if}

  {#if !details.length && !links.length && !userData?.biography}
    <p class="profile-empty">This user hasn't added any profile details yet.</p>
  {/if}
</article>

<style lang="scss">
  .user-profile {
    --profile-line: color-mix(in srgb, currentColor 16%, transparent);
    --profile-tint: color-mix(in srgb, currentColor 4%, transparent);
    --profile-muted: color-mix(in srgb, currentColor 60%, transparent);
    --profile-accent: #1f5fa8;
    --profile-radius: var(--size-border-radius, 6px);

    max-width: 48rem;
    margin-block: 16px;
    overflow: hidden;
    border: 1px solid var(--profile-line);
    border-radius: var(--profile-radius);
  }

  .profile-header {
    display: flex;
    flex-wrap: wrap;
    gap: 16px 20px;
    align-items: center;
    padding: 20px;
    background: var(--profile-tint);
    border-bottom: 1px solid var(--profile-line);
  }

  .profile-avatar {
    flex: none;
    width: 72px;
    height: 72px;
    object-fit: cover;
    border: 1px solid var(--profile-line);
    border-radius: 50%;

    &.is-initial {
      display: grid;
      place-items: center;
      font-size: 2em;
      font-weight: 600;
      color: #fff;
      background: var(--profile-accent);
      border: 0;
    }
  }

  .profile-identity {
    flex: 1 1 16rem;
    min-width: 0;

    h1 {
      margin: 0;
      overflow-wrap: anywhere;
    }
  }

  .profile-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 8px;
    margin: 4px 0 0;
    font-size: 0.9em;
    color: var(--profile-muted);
  }

  .profile-slug {
    font-family: var(--font-mono, monospace);
  }

  .profile-edit {
    padding: 8px 16px;
    font-weight: 600;
    color: var(--profile-accent);
    text-decoration: none;
    border: 1px solid var(--profile-accent);
    border-radius: var(--profile-radius);

    &:hover,
    &:focus-visible {
      color: #fff;
      background: var(--profile-accent);
    }
  }

  .profile-details {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: 16px 24px;
    padding: 20px;
    margin: 0;

    dt {
      font-size: 0.75em;
      font-weight: 600;
      color: var(--profile-muted);
      text-transform: uppercase;
      letter-spacing: 0.06em;
    }

    dd {
      margin: 2px 0 0;
      overflow-wrap: anywhere;
    }
  }

  .biography {
    padding: 0 20px 20px;

    h2 {
      margin: 0 0 6px;
      font-size: 1em;
    }

    p {
      margin: 0;
      white-space: pre-line;
    }
  }

  .profile-empty {
    padding: 20px;
    margin: 0;
    color: var(--profile-muted);
  }
</style>
