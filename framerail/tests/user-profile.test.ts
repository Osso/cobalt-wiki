import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { default: UserProfile } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/user/[slug]/page.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")

const user = {
  user_id: 7570595,
  user_type: "regular",
  created_at: "2021-07-24T14:28:44Z",
  updated_at: "2026-09-24T08:14:19Z",
  deleted_at: null,
  name: "invisibleinkie",
  slug: "invisibleinkie",
  avatar_s3_hash: null,
  website: null,
  user_page: null
}
const blank = {
  name: "",
  realName: "",
  email: "",
  gender: "",
  birthday: "",
  location: "",
  website: "",
  userPage: "",
  biography: "",
  locales: ""
}

function display(overrides: { user?: object; viewer?: number; userData?: object } = {}) {
  const shown = { ...user, ...overrides.user }
  return render(UserProfile, {
    props: {
      data: {
        site: { name: "Cobalt Company" },
        site_file_domain: "cobalt-company.wjfiles.com",
        user_session:
          overrides.viewer === undefined ? null : { user: { user_id: overrides.viewer } },
        user: shown,
        internationalization: { "user-profile-info.location": "Location:" }
      },
      userData: { ...blank, name: shown.name, ...overrides.userData }
    }
  }).body
}

test("public profile shows identity without debug output or edit controls", () => {
  const body = display()
  assert.doesNotMatch(body, /UNTRANSLATED|<textarea|"user_session"/)
  assert.match(body, /<h1[^>]*>invisibleinkie<\/h1>/)
  assert.match(
    body,
    /Account created <time datetime="2021-07-24T14:28:44Z"[^>]*>24 July 2021<\/time>/
  )
  assert.match(body, /hasn't added any profile details/)
  assert.doesNotMatch(body, /Edit profile/)
})

test("own profile offers Edit profile and lists private details", () => {
  const body = display({
    viewer: 7570595,
    userData: {
      realName: "Inky",
      location: "Dallas",
      biography: "Writes the guild canon."
    }
  })
  assert.match(body, /href="\/-\/settings"[^>]*>Edit profile</)
  assert.match(body, /<dt[^>]*>Real name<\/dt>\s*<dd[^>]*>Inky<\/dd>/)
  assert.match(body, /<dt[^>]*>Location<\/dt>\s*<dd[^>]*>Dallas<\/dd>/)
  assert.match(body, /Writes the guild canon\./)
  assert.doesNotMatch(body, /hasn't added any profile details/)
})

test("only http(s) profile URLs become links", () => {
  const scriptUrl = ["javascript", "alert(1)"].join(":")
  const body = display({
    user: { website: "https://example.org/inky", user_page: scriptUrl },
    userData: { website: "https://example.org/inky", userPage: scriptUrl }
  })
  assert.match(
    body,
    /<a href="https:\/\/example\.org\/inky"[^>]*>https:\/\/example\.org\/inky<\/a>/
  )
  assert.doesNotMatch(body, /href="javascript:/)
  assert.match(body, /User page<\/dt>\s*<dd[^>]*>(<!--[^>]*-->)?javascript:alert\(1\)/)
})
