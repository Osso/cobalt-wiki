<script lang="ts">
  import { enhance } from "$app/forms"
  import { resolve } from "$app/paths"

  import type { PageProps } from "./$types"

  let { data, form }: PageProps = $props()

  type SiteMember = PageProps["data"]["members"][number]

  type Tab = "all" | "moderators" | "admins"
  const TABS: { id: Tab; label: string }[] = [
    { id: "all", label: "All Members" },
    { id: "moderators", label: "Moderators" },
    { id: "admins", label: "Administrators" }
  ]
  const ROLE_NAMES: Record<SiteMember["role"], string> = {
    member: "Member",
    moderator: "Moderator",
    admin: "Administrator",
    root: "Site owner"
  }
  const MONTHS = "Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec".split(" ")

  let tab = $state<Tab>("all")
  const shown = $derived(data.members.filter((member) => inTab(member, tab)))
  const loginUrl = `${resolve("/-/login", {})}?origUrl=${encodeURIComponent("/-/admin/members")}`

  function inTab(member: SiteMember, current: Tab): boolean {
    if (current === "moderators") return member.role === "moderator"
    if (current === "admins") return member.role === "admin" || member.role === "root"
    return true
  }

  /** "29 Apr 2021" in UTC, the same on the server and in the browser. */
  function joinDate(iso: string): string {
    const date = new Date(iso)
    return `${date.getUTCDate()} ${MONTHS[date.getUTCMonth()]} ${date.getUTCFullYear()}`
  }

  function changeable(member: SiteMember): boolean {
    return member.role !== "root" && member.user_id !== data.viewerId
  }
</script>

<h1>Site members</h1>

{#if data.access === "signed-out"}
  <p>You need to <a href={loginUrl}>sign in</a> as an administrator of this site.</p>
{:else if data.access === "denied"}
  <p class="members-result error" role="status">
    Only administrators of this site can see and manage its members.
  </p>
{:else}
  {#if form?.message}
    <p class={["members-result", form.saved ? "saved" : "error"]} role="status">
      {form.message}
    </p>
  {/if}

  <div class="members-tabs" role="tablist">
    {#each TABS as { id, label } (id)}
      <button
        class:selected={tab === id}
        aria-selected={tab === id}
        onclick={() => (tab = id)}
        role="tab"
        type="button">{label}</button
      >
    {/each}
  </div>

  <div class="members-table-wrap">
    <table class="members-table">
      <thead>
        <tr>
          <th>Name</th>
          <th>Email</th>
          <th>Role</th>
          <th>Member since</th>
          <th>Options</th>
        </tr>
      </thead>
      <tbody>
        {#each shown as member (member.user_id)}
          <tr>
            <td>
              <a href={resolve("/-/user/[slug]", { slug: member.slug })}>{member.name}</a>
            </td>
            <td class="members-email">
              {member.email.endsWith(".invalid") ? "(not set)" : member.email}
            </td>
            <td>{ROLE_NAMES[member.role]}</td>
            <td>{joinDate(member.joined_at)}</td>
            <td>
              {#if changeable(member)}
                <form class="members-role" action="?/role" method="POST" use:enhance>
                  <input name="userId" type="hidden" value={member.user_id} />
                  <input name="name" type="hidden" value={member.name} />
                  <select
                    name="role"
                    aria-label="Role of {member.name}"
                    value={member.role}
                  >
                    <option value="member">Member</option>
                    <option value="moderator">Moderator</option>
                    <option value="admin">Administrator</option>
                  </select>
                  <button type="submit">Change</button>
                </form>
                <details class="members-remove">
                  <summary>Remove</summary>
                  <form action="?/remove" method="POST" use:enhance>
                    <input name="userId" type="hidden" value={member.user_id} />
                    <input name="name" type="hidden" value={member.name} />
                    <button type="submit">Remove {member.name} from the site</button>
                  </form>
                </details>
              {:else if member.user_id === data.viewerId}
                <span class="members-none">You</span>
              {/if}
            </td>
          </tr>
        {:else}
          <tr><td colspan="5">No members in this list.</td></tr>
        {/each}
      </tbody>
    </table>
  </div>

  <form class="members-invite" action="?/invite" method="POST" use:enhance>
    <h2>Invite a member</h2>
    <p>
      An address with no account gets a new account under the name below and an email with
      a link to choose a password. An existing account joins without an email.
    </p>
    <label>
      Email address
      <input name="email" class="text" autocomplete="off" required type="email" />
    </label>
    <label>
      Name for a new account
      <input name="name" class="text" autocomplete="off" type="text" />
    </label>
    <button type="submit">Invite</button>
  </form>
{/if}

<style lang="scss">
  @use "../../../../lib/css/account-form" as *;

  .members-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25em;
    margin: 1em 0 0;
    border-bottom: 1px solid #8a94a6;

    button {
      padding: 0.4em 1em;
      font: inherit;
      color: #1f5fa8;
      cursor: pointer;
      background: #eef2f8;
      border: 1px solid #8a94a6;
      border-bottom: none;
      border-radius: 4px 4px 0 0;

      &.selected {
        font-weight: bold;
        color: #111;
        background: #fff;
      }
    }
  }

  .members-table-wrap {
    overflow-x: auto;
  }

  .members-table {
    width: 100%;
    border-collapse: collapse;

    th,
    td {
      padding: 0.45em 0.6em;
      vertical-align: top;
      text-align: left;
      border-bottom: 1px solid #d5dae3;
    }

    th {
      white-space: nowrap;
    }
  }

  .members-email {
    overflow-wrap: anywhere;
  }

  .members-role {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3em;
    align-items: center;

    @include account-form-controls("select");

    select,
    button {
      padding: 0.2em 0.5em;
      font-size: 1em;
    }
  }

  .members-remove {
    margin-top: 0.3em;

    summary {
      color: #8a1f1f;
      cursor: pointer;
    }

    form {
      margin-top: 0.3em;

      @include account-form-controls;

      button {
        padding: 0.2em 0.6em;
        font-size: 1em;
        background: #a12a2a;
        border-color: #a12a2a;

        &:hover,
        &:focus-visible {
          background: #bf3535;
          border-color: #bf3535;
        }
      }
    }
  }

  .members-none {
    color: #555;
  }

  .members-invite {
    display: flex;
    flex-direction: column;
    gap: 0.75em;
    max-width: 32em;
    margin: 2em 0 1em;

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

  .members-result {
    padding: 0.5em 0.75em;
    margin: 1em 0;
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
