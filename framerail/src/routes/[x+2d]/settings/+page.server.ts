import {
  loadSettingsPage,
  settingsAvatarAction,
  settingsEmailAction,
  settingsPasswordAction,
  settingsProfileAction,
  settingsWatchingAction
} from "$lib/server/load/settings"
import { setSubscriptionAction } from "$lib/server/load/watching"

export async function load({ request, cookies, parent }) {
  return loadSettingsPage(request, cookies, parent)
}

export const actions = {
  profile: settingsProfileAction,
  avatar: settingsAvatarAction,
  email: settingsEmailAction,
  password: settingsPasswordAction,
  watching: settingsWatchingAction,
  unwatch: setSubscriptionAction
}
