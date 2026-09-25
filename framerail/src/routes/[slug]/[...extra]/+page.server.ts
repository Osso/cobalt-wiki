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
  pageSetTagsAction,
  pageVoteCancelAction,
  pageVoteCastAction,
  pageVoteGetAction
} from "$lib/server/load/page"
import { pagePreviewAction } from "$lib/server/load/page-preview"
import { loadPageWatching, setSubscriptionAction } from "$lib/server/load/watching"
import { loadSiteInfo } from "$lib/server/load/site-info"
import {
  editorPagesAction,
  editorAttachmentsAction
} from "$lib/server/load/editor-lookup"
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
  const { siteId } = loadSiteInfo(request.headers)
  const sessionToken = (await parent()).user_session
    ? cookies.get("wikijump_token")
    : null
  const watching =
    "page" in page
      ? await loadPageWatching(siteId, page.page, sessionToken ?? null)
      : null
  return { ...page, watching }
}

export const actions = {
  delete: pageDeleteAction,
  edit: pageEditAction,
  watching: setSubscriptionAction,
  preview: pagePreviewAction,
  editorPages: editorPagesAction,
  editorAttachments: editorAttachmentsAction,
  draftGet: pageDraftGetAction,
  draftSave: pageDraftSaveAction,
  draftDelete: pageDraftDeleteAction,
  editPermission: pageEditPermissionAction,
  setTags: pageSetTagsAction,
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
