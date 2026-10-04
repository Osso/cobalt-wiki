import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { createServer } from "vite"

// Vite may load a distinct HttpError class; normalize its public status contract.
export async function invokeSsrAction<T>(invoke: () => T) {
  try {
    return await invoke()
  } catch (cause) {
    if (typeof cause !== "object" || cause === null) throw cause
    if (!("status" in cause)) throw cause
    const status = cause.status
    if (typeof status !== "number") throw cause
    const isAccessFailure = [401, 403, 404].includes(status)
    if (!isAccessFailure) throw cause
    return { status }
  }
}

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
