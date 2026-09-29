import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Signs in, uploads a photo-sized avatar from My account, checks the
// resized image in the header and at /-/avatar/<id>, then removes it.
//
// COBALT_AVATAR_URL            site origin, e.g. http://127.0.0.1:3090
// COBALT_EDITOR_USER           account to sign in with (left without an avatar)
// COBALT_EDITOR_PASSWORD_FILE  that account's password
const origin = process.env.COBALT_AVATAR_URL
const account = process.env.COBALT_EDITOR_USER
const passwordFile = process.env.COBALT_EDITOR_PASSWORD_FILE

/**
 * A 1200×800 PNG, drawn in the page so no fixture file is needed.
 *
 * @param {import("@playwright/test").Page} page
 */
async function photoBytes(page) {
  const base64 = await page.evaluate(() => {
    const canvas = document.createElement("canvas")
    canvas.width = 1200
    canvas.height = 800
    const context = canvas.getContext("2d")
    if (!context) throw new Error("canvas 2D required")
    context.fillStyle = "#c33"
    context.fillRect(0, 0, 1200, 800)
    context.fillStyle = "#33c"
    context.fillRect(200, 0, 800, 800)
    return canvas.toDataURL("image/png").split(",")[1]
  })
  return Buffer.from(base64, "base64")
}

test("members upload, see and remove their avatar from My account", async () => {
  assert.ok(origin && account && passwordFile, "origin and account required")
  const browser = await chromium.launch({ executablePath: "/usr/bin/chromium" })
  try {
    const page = await browser.newPage()
    await page.goto(`${origin}/-/login?origUrl=%2F-%2Fsettings`, {
      waitUntil: "networkidle"
    })
    await page.locator('#login [name="nameOrEmail"]').fill(account)
    await page
      .locator('#login [name="password"]')
      .fill((await readFile(passwordFile, "utf8")).trim())
    await page.locator("#login button[type=submit]").click()
    await page.waitForURL(`${origin}/-/settings`)

    const headerAvatar = page.locator("#account-topbutton .user-avatar")
    await expect(headerAvatar).toHaveClass(/is-initials/)
    const photo = await photoBytes(page)
    assert.ok(photo.length > 0)

    await page.locator('input[name="avatar"]').setInputFiles({
      name: "photo.png",
      mimeType: "image/png",
      buffer: photo
    })
    await expect(page.getByAltText("New avatar preview")).toBeVisible()
    const chosen = await page.locator('input[name="avatar"]').evaluate((input) => {
      if (!(input instanceof HTMLInputElement) || !input.files?.[0]) {
        throw new Error("chosen avatar file required")
      }
      return { type: input.files[0].type, size: input.files[0].size }
    })
    assert.equal(chosen.type, "image/webp", "chosen file is replaced by the resized WebP")
    assert.ok(chosen.size < 250 * 1024, `resized avatar fits the limit: ${chosen.size}`)

    await page.getByRole("button", { name: "Upload avatar" }).click()
    await expect(page.locator(".settings-result.saved")).toHaveText(
      "Your settings were saved."
    )
    await expect(headerAvatar.locator("img")).toBeVisible()
    const src = await headerAvatar.locator("img").getAttribute("src")
    assert.ok(src, "header avatar src required")
    assert.match(src, /^\/-\/avatar\/-?\d+\?v=[0-9a-f]{12}$/)

    const served = await page.request.get(`${origin}${src}`)
    assert.equal(served.status(), 200)
    assert.match(served.headers()["content-type"], /^image\/webp/)
    const size = await page.evaluate(async (url) => {
      const bitmap = await createImageBitmap(await (await fetch(url)).blob())
      return [bitmap.width, bitmap.height]
    }, src)
    assert.deepEqual(size, [256, 256])

    await page.getByRole("button", { name: "Remove avatar" }).click()
    await expect(page.locator(".settings-result.saved")).toBeVisible()
    await expect(headerAvatar).toHaveClass(/is-initials/)
    const avatarPath = src.split("?")[0]
    assert.equal((await page.request.get(`${origin}${avatarPath}`)).status(), 404)
  } finally {
    await browser.close()
  }
})
