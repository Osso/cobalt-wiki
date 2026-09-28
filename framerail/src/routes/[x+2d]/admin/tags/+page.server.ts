import {
  loadTagAdminPage,
  tagApplyAction,
  tagPreviewAction,
  tagUndoAction
} from "$lib/server/load/tag-admin"

export async function load({ request, cookies, parent }) {
  return loadTagAdminPage(request, cookies, parent)
}

export const actions = {
  preview: tagPreviewAction,
  apply: tagApplyAction,
  undo: tagUndoAction
}
