import {
  loadSettingsPage,
  settingsEmailAction,
  settingsPasswordAction,
  settingsProfileAction
} from "$lib/server/load/settings"

export async function load({ request, cookies, parent }) {
  return loadSettingsPage(request, cookies, parent)
}

export const actions = {
  profile: settingsProfileAction,
  email: settingsEmailAction,
  password: settingsPasswordAction
}
