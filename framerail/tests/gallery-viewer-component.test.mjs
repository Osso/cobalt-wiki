import assert from "node:assert/strict"
import { test } from "node:test"
import { readFile, unlink, writeFile } from "node:fs/promises"
import { compile } from "svelte/compiler"
import { render } from "svelte/server"

const component = new URL("../src/lib/component/GalleryViewer.svelte", import.meta.url)

async function renderViewer(initialIndex) {
  const compiled = compile(await readFile(component, "utf8"), {
    generate: "server",
    filename: "GalleryViewer.svelte"
  })
  assert.deepEqual(compiled.warnings, [])
  const fixture = new URL(`./.gallery-viewer-${process.pid}.mjs`, import.meta.url)
  await writeFile(fixture, compiled.js.code)
  try {
    const { default: GalleryViewer } = await import(
      `${fixture.href}?index=${initialIndex}`
    )
    return render(GalleryViewer, {
      props: {
        images: [
          { src: "/first-original.png", alt: "First badge" },
          { src: "/second-original.png", alt: "Second badge" }
        ],
        initialIndex,
        onclose() {}
      }
    }).body
  } finally {
    await unlink(fixture)
  }
}

test("viewer shows original image and first-image counter without wrapping backward", async () => {
  const body = await renderViewer(0)
  assert.match(body, /<dialog[^>]*aria-label="Gallery viewer"/)
  assert.match(body, /src="\/first-original.png"/)
  assert.match(body, /alt="First badge"/)
  assert.match(body, /Image 1 of 2/)
  assert.match(body, /<button[^>]*disabled[^>]*>Previous<\/button>/)
  assert.match(body, /<button[^>]*>Next<\/button>/)
  assert.match(body, /<button[^>]*>Close<\/button>/)
})

test("viewer shows selected image and disables next at the end", async () => {
  const body = await renderViewer(1)
  assert.match(body, /src="\/second-original.png"/)
  assert.match(body, /Image 2 of 2/)
  assert.match(body, /<button[^>]*disabled[^>]*>Next<\/button>/)
})
