import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { fileURLToPath } from "node:url"
import * as sass from "sass"
import { preprocess } from "svelte/compiler"
import sveltePreprocess from "svelte-preprocess"

const main = fileURLToPath(new URL("../src/lib/css/main.scss", import.meta.url))
const toasts = fileURLToPath(
  new URL("../src/lib/component/Toasts.svelte", import.meta.url)
)

/** @param {string} css @param {RegExp[]} rules */
function assertSmallScreenRules(css, rules) {
  const mediaQueries = css.match(/@media \(max-width: 850px\)/g) ?? []
  assert.equal(mediaQueries.length, 2)
  for (const rule of rules) assert.match(css, rule)
}

test("main CSS keeps its 850px layout and typography rules without Sass deprecations", () => {
  /** @type {string[]} */
  const deprecations = []
  const css = sass.compile(main, {
    logger: {
      warn(message, options) {
        if (options.deprecation) deprecations.push(message)
      }
    }
  }).css
  assertSmallScreenRules(css, [
    /@media \(max-width: 850px\) \{\s*:root \{\s*--layout-navbar-height: 3rem;\s*--layout-body-side-gap: 0\.5rem;/,
    /@media \(max-width: 850px\) \{\s*:root \{\s*--font-content-size: 0\.75;/
  ])
  assert.deepEqual(deprecations, [])
})

test("Toasts CSS keeps its 850px positioning and sizing without Sass deprecations", async () => {
  /** @type {string[]} */
  const deprecations = []
  const prepared = await preprocess(
    await readFile(toasts, "utf8"),
    sveltePreprocess({
      scss: {
        logger: {
          warn(message, options) {
            if (options.deprecation) deprecations.push(message)
          }
        }
      }
    }),
    { filename: toasts }
  )
  const css = /<style(?:\s[^>]*)?>([\s\S]*?)<\/style>/.exec(prepared.code)?.[1]
  assert.ok(css, "Toasts style must compile")
  assertSmallScreenRules(css, [
    /@media \(max-width: 850px\) \{\s*:global\(\.toasts\) \{\s*align-items: center;\s*width: 100%;\s*margin: 1rem 0;/,
    /@media \(max-width: 850px\) \{\s*:global\(\.toast\) \{\s*width: 90%;\s*min-width: 0;\s*max-width: none;/
  ])
  assert.deepEqual(deprecations, [])
})
