import {
  importedHistoryAction,
  importedRevisionAction,
  layoutAction,
  loadPage,
  pageDeleteAction,
  pageDeletedGetAction,
  pageEditAction,
  pageEditPermissionAction,
  pageFileDeleteAction,
  pageFileEditAction,
  pageFileHistoryAction,
  pageFileListAction,
  pageFileMoveAction,
  pageFileRestoreAction,
  pageFileUploadAction,
  pageHistoryAction,
  pageLockCreateAction,
  pageLockHistoryAction,
  pageLockRemoveAction,
  pageMoveAction,
  pageParentGetAction,
  pageParentSetAction,
  pageRestoreAction,
  pageRevisionAction,
  pageRollbackAction,
  pageScoreAction,
  pageVoteCancelAction,
  pageVoteCastAction,
  pageVoteGetAction
} from "$lib/server/load/page"
import { pagePreviewAction } from "$lib/server/load/page-preview"
import {
  pageDraftGetAction,
  pageDraftSaveAction,
  pageDraftDeleteAction
} from "$lib/server/load/page-draft"

export async function load({ params, request, cookies, parent, locals }) {
  const page = await loadPage(params.slug, params.extra, request, cookies, parent)
  if ("page" in page) {
    locals.documentLayout = page.page.layout ?? (await parent()).site.layout
  }
  return page
}

export const actions = {
  delete: pageDeleteAction,
  edit: pageEditAction,
  preview: pagePreviewAction,
  draftGet: pageDraftGetAction,
  draftSave: pageDraftSaveAction,
  draftDelete: pageDraftDeleteAction,
  editPermission: pageEditPermissionAction,
  fileList: pageFileListAction,
  fileUpload: pageFileUploadAction,
  fileDelete: pageFileDeleteAction,
  fileEdit: pageFileEditAction,
  fileMove: pageFileMoveAction,
  fileRestore: pageFileRestoreAction,
  fileHistory: pageFileHistoryAction,
  history: pageHistoryAction,
  importedHistory: importedHistoryAction,
  importedRevision: importedRevisionAction,
  revision: pageRevisionAction,
  rollback: pageRollbackAction,
  layout: layoutAction,
  lockCreate: pageLockCreateAction,
  lockHistory: pageLockHistoryAction,
  lockRemove: pageLockRemoveAction,
  move: pageMoveAction,
  parentSet: pageParentSetAction,
  parentGet: pageParentGetAction,
  voteGet: pageVoteGetAction,
  voteCast: pageVoteCastAction,
  voteCancel: pageVoteCancelAction,
  score: pageScoreAction,
  deletedGet: pageDeletedGetAction,
  restore: pageRestoreAction
}
