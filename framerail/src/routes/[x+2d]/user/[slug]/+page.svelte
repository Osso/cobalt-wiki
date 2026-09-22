<script lang="ts">
  import Page from "./page.svelte"
  import { userProfileFields } from "$lib/user-data"

  import type { PageProps } from "./$types"

  let { data }: PageProps = $props()
</script>

<Page {data} userData={userProfileFields(data.user)} />
