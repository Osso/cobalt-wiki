import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { test } from "node:test"
import { fileURLToPath, pathToFileURL } from "node:url"

const framerail = fileURLToPath(new URL("../", import.meta.url))
const marker = "csrf-action-marker"

/** @param {string | undefined} environment */
function importProjectConfig(environment) {
  const previous = process.env.FRAMERAIL_ENV
  if (environment === undefined) delete process.env.FRAMERAIL_ENV
  else process.env.FRAMERAIL_ENV = environment
  return import(
    new URL(`../svelte.config.js?environment=${environment}`, import.meta.url).href
  ).finally(() => {
    if (previous === undefined) delete process.env.FRAMERAIL_ENV
    else process.env.FRAMERAIL_ENV = previous
  })
}

/** @param {string} directory */
function writeFixture(directory) {
  mkdirSync(join(directory, "src/routes"), { recursive: true })
  symlinkSync(join(framerail, "node_modules"), join(directory, "node_modules"), "dir")
  writeFileSync(join(directory, "package.json"), '{"type":"module"}')
  writeFileSync(
    join(directory, "svelte.config.js"),
    `import projectConfig from ${JSON.stringify(new URL("../svelte.config.js", import.meta.url).href)}\nexport default { ...projectConfig, kit: { ...projectConfig.kit, adapter: { name: "csrf-fixture", adapt() {} }, files: { routes: "src/routes", appTemplate: "src/app.html" }, outDir: "kit-output" } }`
  )
  writeFileSync(
    join(directory, "vite.config.js"),
    'import { sveltekit } from "@sveltejs/kit/vite"\nexport default { plugins: [sveltekit()] }'
  )
  writeFileSync(
    join(directory, "src/app.html"),
    "<!doctype html><html><head>%sveltekit.head%</head><body>%sveltekit.body%</body></html>"
  )
  writeFileSync(
    join(directory, "src/routes/+page.server.js"),
    `export const actions = { default: async () => ({ marker: "${marker}" }) }`
  )
  writeFileSync(
    join(directory, "src/routes/+page.svelte"),
    '<script>let { form } = $props()</script><form method="POST"><button>Submit</button></form><p>{form?.marker ?? "no-action"}</p>'
  )
}

/** @param {string} directory @param {string | undefined} environment */
function runFixtureProcess(directory, environment) {
  const env = { ...process.env }
  if (environment === undefined) delete env.FRAMERAIL_ENV
  else env.FRAMERAIL_ENV = environment
  const run = spawnSync(
    process.execPath,
    [fileURLToPath(import.meta.url), "--fixture", directory],
    {
      cwd: directory,
      env,
      encoding: "utf8",
      timeout: 120_000
    }
  )
  const log = join(tmpdir(), `cobalt-csrf-${environment ?? "unset"}-${process.pid}.log`)
  writeFileSync(
    log,
    `exit: ${run.status}\nerror: ${run.error?.stack ?? "none"}\nstdout:\n${run.stdout}\nstderr:\n${run.stderr}`
  )
  assert.equal(run.status, 0, `Kit fixture failed; full output: ${log}`)
  return JSON.parse(readFileSync(join(directory, "responses.json"), "utf8"))
}

/** @param {string} directory */
async function buildFixture(directory) {
  const { build } = await import("vite")
  const outDir = join(directory, "kit-output")
  await build({
    root: directory,
    configFile: join(directory, "vite.config.js"),
    logLevel: "warn"
  })
  return join(outDir, "output/server")
}

/** @param {string} serverDirectory */
async function loadFixtureServer(serverDirectory) {
  const generated = readdirSync(serverDirectory)
  assert.ok(
    generated.includes("index.js") && generated.includes("manifest.js"),
    `Generated server files: ${generated.join(", ")}`
  )
  const { Server } = await import(pathToFileURL(join(serverDirectory, "index.js")).href)
  const { manifest } = await import(
    pathToFileURL(join(serverDirectory, "manifest.js")).href
  )
  const server = new Server(manifest)
  await server.init({ env: process.env })
  return server
}

/** @param {Awaited<ReturnType<typeof loadFixtureServer>>} server */
async function postOrigins(server) {
  const responses = []
  for (const origin of ["https://fixture.test", "https://foreign.test"]) {
    const request = new Request("https://fixture.test/", {
      method: "POST",
      headers: { origin, "content-type": "application/x-www-form-urlencoded" },
      body: "field=value"
    })
    const response = await server.respond(request, {
      getClientAddress: () => "127.0.0.1"
    })
    responses.push({
      origin,
      status: response.status,
      containsMarker: (await response.text()).includes(marker)
    })
  }
  return responses
}

/** @param {string} directory */
async function buildAndRespond(directory) {
  const serverDirectory = await buildFixture(directory)
  const server = await loadFixtureServer(serverDirectory)
  const responses = await postOrigins(server)
  writeFileSync(join(directory, "responses.json"), JSON.stringify(responses))
}

if (process.argv[2] === "--fixture") {
  await buildAndRespond(process.argv[3])
} else {
  test("CSRF config uses trusted origins instead of deprecated checkOrigin", async () => {
    for (const environment of ["local", "production", undefined]) {
      const { default: actual } = await importProjectConfig(environment)
      assert.deepEqual(
        actual.kit.csrf,
        {
          trustedOrigins: environment === "local" ? ["*"] : []
        },
        `FRAMERAIL_ENV=${environment ?? "unset"}`
      )
    }
  })

  for (const environment of ["local", "production", undefined]) {
    test(`actual Kit form POST policy for FRAMERAIL_ENV=${environment ?? "unset"}`, () => {
      const directory = mkdtempSync(join(tmpdir(), "cobalt-csrf-"))
      try {
        writeFixture(directory)
        const responses = runFixtureProcess(directory, environment)
        assert.deepEqual(responses, [
          { origin: "https://fixture.test", status: 200, containsMarker: true },
          {
            origin: "https://foreign.test",
            status: environment === "local" ? 200 : 403,
            containsMarker: environment === "local"
          }
        ])
      } finally {
        rmSync(directory, { recursive: true, force: true })
      }
    })
  }
}
