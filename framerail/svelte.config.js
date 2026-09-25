import adapter from "@sveltejs/adapter-node"
import { statSync } from "fs"
import { dirname, resolve } from "path"
import { sveltePreprocess } from "svelte-preprocess"
import { fileURLToPath } from "url"

// The former only works on node 20.11+
const __dirname = import.meta.dirname ?? dirname(fileURLToPath(import.meta.url))

function resolveAssets() {
  try {
    let globalAssets = statSync(resolve(__dirname, "../assets"))
    if (globalAssets.isDirectory()) return resolve(__dirname, "../assets")
    else return resolve(__dirname, "src/assets")
  } catch (error) {
    return resolve(__dirname, "src/assets")
  }
}

/** @type {import("@sveltejs/kit").Config} */
const config = {
  // Consult https://github.com/sveltejs/svelte-preprocess
  // for more information about preprocessors
  preprocess: sveltePreprocess(),

  kit: {
    adapter: adapter(),
    appDir: "-/assets",
    csrf: {
      // Local has no fixed DNS; other environments retain strict origin checks.
      trustedOrigins: process.env.FRAMERAIL_ENV === "local" ? ["*"] : []
    },
    alias: {
      "$static": resolve(__dirname, "static"),
      "$assets": resolveAssets()
    }
  }
}

export default config
