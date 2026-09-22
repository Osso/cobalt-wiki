import assert from "node:assert/strict"
import { test } from "node:test"
import { createJiti } from "jiti"
import type { UserModel } from "../src/lib/types"

const jiti = createJiti(import.meta.url, {
  alias: { "$lib": new URL("../src/lib", import.meta.url).pathname }
})
const { UserType } =
  await jiti.import<typeof import("../src/lib/types")>("../src/lib/types")
const { sanitizeUserData, userProfileFields } =
  await jiti.import<typeof import("../src/lib/user-data")>("../src/lib/user-data")

const user: UserModel = {
  user_id: 42,
  user_type: UserType.Regular,
  created_at: "2026-01-01",
  updated_at: null,
  deleted_at: null,
  from_wikidot: false,
  name: "Ada",
  slug: "ada",
  name_changes_left: 2,
  last_name_change_added_at: "2026-01-01",
  last_renamed_at: null,
  email: "ada@example.test",
  email_verified_at: null,
  email_validation_info: null,
  email_validation_at: null,
  password: "password-hash",
  multi_factor_secret: "mfa-secret",
  multi_factor_recovery_codes: ["recovery-secret"],
  locales: ["en", "fr"],
  avatar_s3_hash: null,
  real_name: "Ada Example",
  gender: null,
  birthday: null,
  location: "Private location",
  biography: "Private biography",
  website: "https://example.test",
  user_page: "profile:ada"
}

test("public sanitization retains only public fields and supports profile rendering", () => {
  const sanitized = sanitizeUserData(user, true)
  assert.deepEqual(
    Object.keys(sanitized).sort(),
    [
      "user_id",
      "user_type",
      "created_at",
      "updated_at",
      "deleted_at",
      "name",
      "slug",
      "avatar_s3_hash",
      "website",
      "user_page"
    ].sort()
  )
  assert.deepEqual(userProfileFields(sanitized), {
    name: "Ada",
    website: "https://example.test",
    userPage: "profile:ada",
    realName: "",
    email: "",
    gender: "",
    birthday: "",
    location: "",
    biography: "",
    locales: ""
  })
  assert.equal(user.password, "password-hash")
})

test("self sanitization retains account fields but never credential fields", () => {
  const sanitized = sanitizeUserData(user, false)
  assert.equal(sanitized.email, "ada@example.test")
  assert.deepEqual(sanitized.locales, ["en", "fr"])
  assert.equal(sanitized.real_name, "Ada Example")
  for (const key of [
    "password",
    "multi_factor_secret",
    "multi_factor_recovery_codes",
    "from_wikidot"
  ]) {
    assert.equal(key in sanitized, false)
  }
  assert.equal(userProfileFields(sanitized).locales, "en fr")
  assert.equal(userProfileFields(sanitized).realName, "Ada Example")
})

test("missing profile produces empty display fields without credentials", () => {
  assert.equal(userProfileFields(undefined).name, "")
  assert.equal(userProfileFields(undefined).email, "")
})
