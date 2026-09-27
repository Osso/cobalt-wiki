// Sign-in persistence against the local stack: a plain sign-in lasts until the
// browser closes; "Keep me signed in" survives restarts; sign-out clears both.
import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

/** @typedef {import("@playwright/test").Browser} Browser */
/** @typedef {import("@playwright/test").BrowserContext} BrowserContext */
/** @typedef {import("@playwright/test").Cookie} Cookie */

const origin = "http://127.0.0.1:3090"
const deepwell = "http://127.0.0.1:2749/jsonrpc"
const fixturePath =
  "/home/osso/.local/share/cobalt-wiki/local-full/page-action-fixture.json"
const DAY_MS = 24 * 60 * 60 * 1000

/**
 * @param {BrowserContext} context @param {string} username @param {string}
 *   password @param {boolean} remember
 */
async function signIn(context, username, password, remember) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await page.locator('#login [name="nameOrEmail"]').fill(username)
  await page.locator('#login [name="password"]').fill(password)
  const keep = page.getByLabel("Keep me signed in")
  await expect(keep, "Keep me signed in starts checked").toBeChecked()
  if (!remember) await keep.uncheck()
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  return page
}

/** @param {BrowserContext} context */
async function sessionCookies(context) {
  const cookies = (await context.cookies()).filter(
    (cookie) => cookie.domain === "127.0.0.1"
  )
  return {
    token: cookies.find((cookie) => cookie.name === "wikijump_token"),
    remember: cookies.find((cookie) => cookie.name === "wikijump_remember")
  }
}

/**
 * Reopens the browser: only persistent cookies (expires > 0) survive.
 *
 * @param {Browser} browser @param {Cookie[]} cookies
 */
async function restart(browser, cookies) {
  const context = await browser.newContext()
  await context.addCookies(cookies.filter((cookie) => cookie.expires > 0))
  return context
}

/** @param {BrowserContext} context */
async function isSignedIn(context) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  const signedIn = (await page.locator("#login").count()) === 0
  await page.close()
  return signedIn
}

/** @param {string} token */
async function serverExpiry(token) {
  const response = await fetch(deepwell, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "session_get",
      params: [decodeURIComponent(token)]
    })
  })
  const { result } = await response.json()
  assert.ok(result, "server session exists")
  return Date.parse(result.expires_at)
}

test("sign-in lasts for the browser session, or across restarts when remembered", async () => {
  const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(passwordPath, "admin password file path required")
  const password = (await readFile(passwordPath, "utf8")).trim()
  const { username } = JSON.parse(await readFile(fixturePath, "utf8"))
  const browser = await chromium.launch({ executablePath: "/usr/bin/chromium" })
  try {
    const plain = await browser.newContext()
    await signIn(plain, username, password, false)
    const plainCookies = await sessionCookies(plain)
    assert.ok(plainCookies.token, "plain sign-in sets the token")
    assert.equal(
      plainCookies.token.expires,
      -1,
      "plain token is a browser-session cookie"
    )
    assert.equal(plainCookies.remember, undefined)
    assert.ok(
      (await serverExpiry(plainCookies.token.value)) > Date.now() + DAY_MS,
      "server session outlives an idle afternoon"
    )
    const plainReopened = await restart(browser, await plain.cookies())
    assert.equal(await isSignedIn(plainReopened), false, "closing the browser signs out")

    const remembered = await browser.newContext()
    await signIn(remembered, username, password, true)
    const kept = await sessionCookies(remembered)
    for (const cookie of [kept.token, kept.remember]) {
      assert.ok(cookie, "remembered sign-in sets both cookies")
      const days = (cookie.expires * 1000 - Date.now()) / DAY_MS
      assert.ok(days > 399 && days <= 400, `${cookie.name} lasts 400 days, got ${days}`)
    }
    const rememberedReopened = await restart(browser, await remembered.cookies())
    assert.equal(
      await isSignedIn(rememberedReopened),
      true,
      "remembered session survives"
    )

    const page = await rememberedReopened.newPage()
    await page.goto(`${origin}/-/logout`, { waitUntil: "networkidle" })
    await page.locator(".button-logout").click()
    await expect
      .poll(
        async () =>
          Object.values(await sessionCookies(rememberedReopened)).filter(Boolean).length
      )
      .toBe(0)
  } finally {
    await browser.close()
  }
})
