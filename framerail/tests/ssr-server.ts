import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { createServer } from "vite"

export async function createSsrServer() {
  const cacheDir = await mkdtemp(join(tmpdir(), "cobalt-ssr-vite-"))
  try {
    const vite = await createServer({
      cacheDir,
      server: { middlewareMode: true, ws: false },
      optimizeDeps: { noDiscovery: true, include: [] },
      logLevel: "error"
    })
    return {
      vite,
      close: async () => {
        try {
          await vite.close()
        } finally {
          await rm(cacheDir, { recursive: true, force: true })
        }
      }
    }
  } catch (error) {
    await rm(cacheDir, { recursive: true, force: true })
    throw error
  }
}
