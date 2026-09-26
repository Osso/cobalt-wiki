import assert from "node:assert/strict"
import { expect } from "@playwright/test"
import {
  origin,
  listFiles,
  readFileBytes,
  assertActionSuccess
} from "./file-action-transport.mjs"

/**
 * @typedef {Awaited<
 *   ReturnType<
 *     typeof import("./file-action-transport.mjs").guardBrowserWrites
 *   >
 * >} WriteGuard
 */

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function openFiles(page, slug) {
  const response = await page.goto(`${origin}/${slug}`, { waitUntil: "networkidle" })
  assert.equal(response?.status(), 200)
  const wikijumpFiles = page.locator(".other-actions .button-files")
  if (await wikijumpFiles.isVisible()) {
    await wikijumpFiles.click()
  } else {
    await page.locator("#page-options-bottom #files-button").click()
  }
  const pane = page.locator(".file-panel")
  await expect(pane).toBeVisible()
  return pane
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {string} action @param
 *   {import("@playwright/test").Locator} control
 * @param {number | null} [fileId] @param {string | null} [name]
 */
async function clickMutation(
  page,
  guard,
  slug,
  action,
  control,
  fileId = null,
  name = null
) {
  const count = guard.writes.length
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() === `${origin}/${slug}?/${action}` &&
      response.request().method() === "POST"
  )
  guard.allow(slug, action, fileId, name)
  const [response] = await Promise.all([responsePromise, control.click()])
  await assertActionSuccess(response, action)
  assert.equal(guard.writes.length, count + 1, `${action} dispatch count`)
  guard.assertConsumed()
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {number} pageId @param
 *   {number} fileId @param {string} name @param {Buffer} bytes @param
 *   {number} priorRevision
 */
async function assertFile(
  request,
  token,
  slug,
  pageId,
  fileId,
  name,
  bytes,
  priorRevision = 0
) {
  const { file, bytes: actual } = await readFileBytes(
    request,
    token,
    slug,
    pageId,
    fileId
  )
  assert.equal(file.name, name)
  assert.equal(file.revision_type === "delete", false)
  assert.ok(file.revision_id > priorRevision, "file revision must advance")
  assert.deepEqual(actual, bytes)
  const active = await listFiles(request, token, slug, pageId, false)
  assert.equal(active.filter((entry) => entry.file_id === fileId).length, 1)
  return file
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId @param {string} action
 */
async function openFileAction(page, guard, slug, fileId, action) {
  const pane = await openFiles(page, slug)
  const row = pane.locator(`.file-row[data-id="${fileId}"]`)
  await expect(row).toBeVisible()
  const selector = action === "history" ? ".file-history" : `.${action}-file`
  const button = row.locator(selector)
  if (await button.count()) {
    await button.click()
  } else {
    await row
      .locator(".action a", {
        hasText: action === "edit" ? "Edit" : action === "move" ? "Move" : "History"
      })
      .click()
  }
  return pane
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId @param {string} name
 * @param {Buffer | null} bytes
 */
async function editInBrowser(page, guard, slug, fileId, name, bytes) {
  const pane = await openFileAction(page, guard, slug, fileId, "edit")
  const form = pane.locator("#file-edit")
  await expect(form).toBeVisible()
  await form.locator('[name="name"]').fill(name)
  if (bytes) {
    await form.locator('[name="file"]').setInputFiles({
      name,
      mimeType: "text/plain",
      buffer: bytes
    })
  }
  await clickMutation(
    page,
    guard,
    slug,
    "fileEdit",
    form.locator('[type="submit"]'),
    fileId
  )
  await expect(form).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId @param {string} destination
 */
async function moveInBrowser(page, guard, slug, fileId, destination) {
  const pane = await openFileAction(page, guard, slug, fileId, "move")
  const form = pane.locator("#file-move")
  await form.locator('[name="destinationPage"]').fill(destination)
  await clickMutation(
    page,
    guard,
    slug,
    "fileMove",
    form.locator('[type="submit"]'),
    fileId
  )
  await expect(form).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId
 */
async function deleteInBrowser(page, guard, slug, fileId) {
  const pane = await openFiles(page, slug)
  const row = pane.locator(`.file-row[data-id="${fileId}"]`)
  await expect(row).toBeVisible()
  const button = row.locator(".delete-file, .action a:has-text('Delete')")
  await clickMutation(page, guard, slug, "fileDelete", button, fileId)
  await expect(row).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId
 */
async function restoreInBrowser(page, guard, slug, fileId) {
  const pane = await openFiles(page, slug)
  await pane.locator(".deleted-file, .buttons input[value='Restore']").click()
  const row = pane.locator(`.file-row[data-id="${fileId}"]`)
  const restore = row.locator(".restore-file, .action a:has-text('Restore')")
  await expect(restore).toBeVisible()
  await restore.click()
  const form = pane.locator("#file-restore")
  await clickMutation(
    page,
    guard,
    slug,
    "fileRestore",
    form.locator('[type="submit"]'),
    fileId
  )
  await expect(form).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId @param {number}
 *   revisionNumber
 */
async function rollbackInBrowser(page, guard, slug, fileId, revisionNumber) {
  const pane = await openFileAction(page, guard, slug, fileId, "history")
  await expect(pane.locator(".revision-list")).toBeVisible()
  const row = pane.locator(".revision-row").filter({
    has: page.locator(`.revision-number:text-is("${revisionNumber}")`)
  })
  await expect(row).toHaveCount(1)
  const rollback = row.locator(".revision-rollback, .action a")
  await clickMutation(page, guard, slug, "fileRollback", rollback, fileId)
}

export {
  openFiles,
  clickMutation,
  assertFile,
  editInBrowser,
  moveInBrowser,
  deleteInBrowser,
  restoreInBrowser,
  rollbackInBrowser
}
