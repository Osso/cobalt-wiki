import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs"
import { createServer } from "node:http"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { test } from "node:test"
import { fileURLToPath, pathToFileURL } from "node:url"

const framerail = fileURLToPath(new URL("../", import.meta.url))
const repository = resolve(fileURLToPath(new URL("../../", import.meta.url)))

function runtimeEnvironment() {
  const expression = `let
    flake = builtins.getFlake ${JSON.stringify(repository)};
    system = flake.inputs.nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [ ${join(repository, "install/nixos/module.nix")} {
        services.cobaltWiki.enable = true;
        services.cobaltWiki.mainDomain = "fixture.test";
        services.cobaltWiki.environmentFile = "/run/fixture.env";
      } ];
    };
  in system.config.systemd.services.cobalt-wiki-framerail.environment`
  const result = spawnSync("nix", ["eval", "--json", "--impure", "--expr", expression], {
    cwd: repository,
    encoding: "utf8",
    timeout: 120_000
  })
  assert.equal(result.status, 0, result.stderr)
  return JSON.parse(result.stdout)
}

function writeFixture(directory) {
  mkdirSync(join(directory, "src/routes"), { recursive: true })
  symlinkSync(join(framerail, "node_modules"), join(directory, "node_modules"), "dir")
  writeFileSync(join(directory, "package.json"), '{"type":"module"}')
  writeFileSync(
    join(directory, "svelte.config.js"),
    'import adapter from "@sveltejs/adapter-node"; export default { kit: { adapter: adapter() } }'
  )
  writeFileSync(
    join(directory, "vite.config.js"),
    'import { sveltekit } from "@sveltejs/kit/vite"; export default { plugins: [sveltekit()] }'
  )
  writeFileSync(
    join(directory, "src/app.html"),
    "<!doctype html><html><head>%sveltekit.head%</head><body>%sveltekit.body%</body></html>"
  )
  writeFileSync(
    join(directory, "src/routes/+server.js"),
    'export async function POST({ request }) { const form = await request.formData(); return Response.json({ size: form.get("file").size }); }'
  )
}

async function exerciseAdapter(directory) {
  const { build } = await import("vite")
  await build({
    root: directory,
    configFile: join(directory, "vite.config.js"),
    logLevel: "warn"
  })
  const { handler } = await import(
    pathToFileURL(join(directory, "build/handler.js")).href
  )
  const server = createServer(handler)
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve))
  const address = server.address()
  const url = `http://127.0.0.1:${address.port}/`
  try {
    const fileSize = 578 * 1024
    const form = new FormData()
    form.set(
      "file",
      new File([new Uint8Array(fileSize)], "letter.png", { type: "image/png" })
    )
    const response = await fetch(url, {
      method: "POST",
      headers: { origin: process.env.ORIGIN },
      body: form
    })
    const accepted = { status: response.status, body: await response.text() }
    // Content-Length alone triggers the adapter's finite bound, without sending 100 MiB.
    const oversized = await new Promise((resolve, reject) => {
      import("node:http").then(({ request }) => {
        const req = request(
          url,
          {
            method: "POST",
            headers: {
              origin: process.env.ORIGIN,
              "content-type": "multipart/form-data; boundary=probe",
              "content-length": 100 * 1024 * 1024 + 1
            }
          },
          (res) => {
            res.resume()
            res.on("end", () => resolve(res.statusCode))
          }
        )
        req.on("error", reject)
        req.end("--probe--\r\n")
      }, reject)
    })
    writeFileSync(
      join(directory, "responses.json"),
      JSON.stringify({ accepted, oversized })
    )
  } finally {
    server.closeAllConnections()
    await new Promise((resolve) => server.close(resolve))
  }
}

if (process.argv[2] === "--fixture") {
  await exerciseAdapter(process.argv[3])
} else {
  test("native Framerail accepts the reported 578 KiB multipart image and retains a finite 100 MiB bound", () => {
    const directory = mkdtempSync(join(tmpdir(), "cobalt-upload-limit-"))
    try {
      writeFixture(directory)
      const configured = runtimeEnvironment()
      const environment = { ...process.env, ...configured }
      delete environment.BODY_SIZE_LIMIT
      if (configured.BODY_SIZE_LIMIT !== undefined)
        environment.BODY_SIZE_LIMIT = configured.BODY_SIZE_LIMIT
      const result = spawnSync(
        process.execPath,
        [fileURLToPath(import.meta.url), "--fixture", directory],
        {
          cwd: directory,
          env: environment,
          encoding: "utf8",
          timeout: 120_000
        }
      )
      const log = join(tmpdir(), `cobalt-upload-limit-${process.pid}.log`)
      writeFileSync(log, `${result.stdout}\n${result.stderr}\n${result.error ?? ""}`)
      assert.equal(result.status, 0, `Adapter fixture failed: ${log}`)
      const { accepted, oversized } = JSON.parse(
        readFileSync(join(directory, "responses.json"), "utf8")
      )
      assert.equal(accepted.status, 200, accepted.body)
      assert.deepEqual(JSON.parse(accepted.body), { size: 578 * 1024 })
      assert.equal(oversized, 413)
    } finally {
      rmSync(directory, { recursive: true, force: true })
    }
  })
}
