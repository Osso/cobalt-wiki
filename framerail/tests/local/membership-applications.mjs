import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000

async function rpc(method, params, token) {
  const headers = {
    "Content-Type": "application/json",
    "X-Deepwell-Site-Id": String(siteId),
    "X-Deepwell-Page": "home:_public"
  }
  if (token) headers["X-Deepwell-Session-Token"] = token
  const response = await fetch(backend, {
    method: "POST",
    headers,
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params })
  })
  assert.equal(response.status, 200)
  const body = await response.json()
  assert.ok(!body.error, `${method} failed: ${body.error?.message}`)
  return body.result
}

async function login(browser, name, password) {
  const context = await browser.newContext()
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await page.locator('#login [name="nameOrEmail"]').fill(name)
  await page.locator('#login [name="password"]').fill(password)
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token"
  )
  assert.ok(cookie)
  return { context, page, token: decodeURIComponent(cookie.value) }
}

test("local guest applies, is rejected, reapplies, and gains edit access only after admin approval", async () => {
  const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(passwordPath, "explicit local admin password file required")
  const name = `ApplicationProof${randomBytes(5).toString("hex")}`
  const password = randomBytes(24).toString("base64url")
  const created = await rpc("user_create", {
    user_type: "regular",
    name,
    email: `${name.toLowerCase()}@example.com`,
    locales: ["en"],
    password,
    bypass_filter: true,
    bypass_email_verification: true,
    ip_address: "127.0.0.1"
  })
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const guest = await login(browser, name, password)
    const admin = await login(
      browser,
      "cobalt-import",
      (await readFile(passwordPath, "utf8")).trim()
    )
    assert.equal((await rpc("page_edit_permission", {}, guest.token)).can_edit, false)
    await guest.page.goto(origin, { waitUntil: "networkidle" })
    const join = guest.page.locator(".join-box a")
    await expect(join).toHaveAttribute("href", "/-/join")
    await join.click()
    await expect(
      guest.page.getByRole("heading", { name: "Apply to become a member", exact: true })
    ).toBeVisible()
    await guest.page
      .locator('[name="message"]')
      .fill("I would like to help document our adventures.")
    await guest.page.getByRole("button", { name: "Submit application" }).click()
    await expect(guest.page.getByRole("status")).toContainText(
      "awaiting administrator review"
    )
    assert.equal((await rpc("page_edit_permission", {}, guest.token)).can_edit, false)
    await guest.page.reload()
    await expect(guest.page.locator("textarea")).toHaveCount(0)
    await admin.page.goto(`${origin}/-/admin/members`, { waitUntil: "networkidle" })
    let application = admin.page
      .locator(".membership-application")
      .filter({ hasText: name })
    await expect(application).toContainText(
      "I would like to help document our adventures."
    )
    await application.getByRole("button", { name: "Reject", exact: true }).click()
    await expect(application).toHaveCount(0)
    assert.equal((await rpc("page_edit_permission", {}, guest.token)).can_edit, false)
    await guest.page.reload()
    await guest.page
      .locator('[name="message"]')
      .fill("A revised application for membership.")
    await guest.page.getByRole("button", { name: "Submit application" }).click()
    await expect(guest.page.getByRole("status")).toContainText(
      "awaiting administrator review"
    )
    await admin.page.reload()
    application = admin.page.locator(".membership-application").filter({ hasText: name })
    await application.getByRole("button", { name: "Approve", exact: true }).click()
    await expect(application).toHaveCount(0)
    assert.equal((await rpc("page_edit_permission", {}, guest.token)).can_edit, true)
    await guest.page.reload()
    await expect(guest.page.locator("#page-content")).toContainText("already a member")
    await guest.page.goto(origin, { waitUntil: "networkidle" })
    await guest.page.locator("#edit-button").click()
    await expect(guest.page.locator("#editor")).toBeVisible()
    // Never save source content. Remove only the sacrificial membership via the supported admin RPC.
    await rpc(
      "member_admin_remove",
      { user_id: created.user_id, ip_address: "127.0.0.1" },
      admin.token
    )
    assert.equal((await rpc("page_edit_permission", {}, guest.token)).can_edit, false)
    console.log(`Sacrificial local guest ${name}: no remaining membership, no page saves`)
  } finally {
    await browser.close()
  }
})
