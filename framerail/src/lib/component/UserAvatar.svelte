<!--
  @component A member's avatar: their uploaded image, or their initials.

  `avatarHash` is the user's `avatar_s3_hash` when the caller has it: `null`
  or an empty array means no avatar, and the hash busts caches after a change.
  Lists that only know the user ID omit it; a 404 then shows the initials.
-->
<script lang="ts">
  let {
    userId,
    name,
    avatarHash,
    size = 24
  }: {
    userId: number
    name: string
    avatarHash?: number[] | null
    size?: number
  } = $props()

  let failed = $state(false)

  const known = $derived(avatarHash !== undefined)
  const hasImage = $derived(!failed && (!known || (avatarHash?.length ?? 0) > 0))
  const version = $derived(
    avatarHash?.length
      ? `?v=${avatarHash
          .slice(0, 6)
          .map((byte) => byte.toString(16).padStart(2, "0"))
          .join("")}`
      : ""
  )
  const initials = $derived(
    name
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => [...word][0]?.toUpperCase() ?? "")
      .join("")
  )
</script>

<span
  style:--avatar-size="{size}px"
  class="user-avatar"
  class:is-initials={!hasImage}
  aria-hidden="true"
>
  {#if hasImage}
    <img alt="" onerror={() => (failed = true)} src="/-/avatar/{userId}{version}" />
  {:else}
    {initials}
  {/if}
</span>

<style>
  .user-avatar {
    box-sizing: border-box;
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--avatar-size);
    height: var(--avatar-size);
    overflow: hidden;
    font:
      600 calc(var(--avatar-size) * 0.42) / 1 system-ui,
      -apple-system,
      "Segoe UI",
      sans-serif;
    vertical-align: middle;
    color: #fff;
    letter-spacing: 0.03em;
    border-radius: 50%;
  }

  .is-initials {
    background: linear-gradient(135deg, #3f7fcf, #1f4aa8);
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
