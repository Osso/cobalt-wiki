import { sveltekit } from "@sveltejs/kit/vite"

import type { UserConfig } from "vite"

import svelteConfig from "./svelte.config.js"
import baseConfig from "./vite.config"

const { kit, ...nonKitSvelteOptions } = svelteConfig

const config: UserConfig = {
  ...baseConfig,
  cacheDir: ".vite-history-pilot",
  plugins: [
    sveltekit({
      ...kit,
      ...nonKitSvelteOptions,
      outDir: ".svelte-kit-history-pilot"
    })
  ]
}

export default config
