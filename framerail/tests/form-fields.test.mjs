import assert from "node:assert/strict"
import { test } from "node:test"
import { readFile, writeFile, unlink } from "node:fs/promises"
import { compile } from "svelte/compiler"
import { render } from "svelte/server"
import { createJiti } from "jiti"

const { createDraft, changedFields } =
  /** @type {typeof import("../src/lib/form-editor")} */ (
    await createJiti(import.meta.url).import("../src/lib/form-editor")
  )

test("source-defined fields render accessible typed controls and readonly static text", async () => {
  const source = await readFile(
    new URL("../src/lib/component/DataFormFields.svelte", import.meta.url),
    "utf8"
  )
  const compiled = compile(source, {
    generate: "server",
    filename: "DataFormFields.svelte"
  })
  assert.deepEqual(compiled.warnings, [])
  const fixture = new URL(`./.form-fields-${process.pid}.mjs`, import.meta.url)
  await writeFile(
    fixture,
    compiled.js.code.replaceAll('"../form-editor"', '"../src/lib/form-editor.ts"')
  )
  try {
    const { default: Component } = await import(fixture.href)
    /** @type {import("../src/lib/form-editor").PageForm} */
    const form = {
      schema: {
        properties: {},
        fields: [
          {
            name: "intro",
            kind: "static",
            properties: { value: "Read only <safe>" },
            options: []
          },
          {
            name: "name",
            kind: "text",
            properties: {
              label: "Name",
              hint: "Your player name",
              width: 40,
              after: 'Optional "cover" image. Leave blank for no image. <unsafe>'
            },
            options: []
          },
          {
            name: "bio",
            kind: "wiki",
            properties: {
              label: "Biography",
              hint: "Enter wiki source",
              default: "**hello**",
              width: 80,
              height: 8,
              after: "Plain source only"
            },
            options: []
          },
          {
            name: "rank",
            kind: "select",
            properties: { label: "Rank", hint: "Choose rank", after: "Choose one" },
            options: [
              { code: 1, label: "One" },
              { code: "1", label: "String one" },
              { code: null, label: "Null" },
              { code: "null", label: "String null" }
            ]
          },
          {
            name: "sex",
            kind: "select",
            properties: { label: "Sex" },
            options: [
              { code: false, label: "Female" },
              { code: true, label: "Male" }
            ]
          },
          {
            name: "race",
            kind: "select",
            properties: { label: "Race" },
            options: [
              { code: "human", label: "Human" },
              { code: "elf", label: "Elf" },
              { code: "gnome", label: "Gnome" },
              { code: "orc", label: "Orc" },
              { code: "troll", label: "Troll" }
            ]
          },
          {
            name: "summary-label",
            kind: "static",
            properties: { label: "Summary <script>", value: "Summary value <safe>" },
            options: []
          },
          {
            name: "empty-static",
            kind: "static",
            properties: { label: "", value: "Empty-label value" },
            options: []
          },
          {
            name: "missing-static",
            kind: "static",
            properties: { value: "Missing-label value" },
            options: []
          },
          {
            name: "summary",
            kind: "text",
            properties: { label: "Summary", width: 80, height: 3, default: "A summary" },
            options: []
          },
          {
            name: "single",
            kind: "text",
            properties: { label: "Single", height: 1, after: "" },
            options: []
          },
          {
            name: "legacy-hint",
            kind: "text",
            properties: { label: "Legacy hint", Hint: "Unsupported alias" },
            options: []
          }
        ]
      },
      values: { name: 7, rank: "1", sex: false, race: "orc", unknown: true }
    }
    const draft = createDraft(form)
    const { body } = render(Component, { props: { form, draft } })
    assert.match(body, /Read only &lt;safe(?:&gt;|>)/)
    assert.match(body, /Summary &lt;script(?:&gt;|>)/)
    assert.match(body, /Summary value &lt;safe(?:&gt;|>)/)
    assert.ok(body.indexOf("Summary &lt;script") < body.indexOf("Summary value &lt;safe"))
    assert.doesNotMatch(body, /<script>|summary-label|empty-static|missing-static/)
    assert.match(body, /Empty-label value/)
    assert.match(body, /Missing-label value/)
    assert.equal((body.match(/class="static-label(?:\s[^"]*)?"/g) ?? []).length, 1)
    assert.equal(draft["summary-label"], undefined)
    assert.equal(draft["empty-static"], undefined)
    assert.equal(draft["missing-static"], undefined)
    assert.deepEqual(changedFields(form, { ...draft, "summary-label": "changed" }), {})
    assert.match(body, /<label for="data-form-field-1">Name<\/label>/)
    assert.match(body, /<input[^>]*type="text"/)
    assert.match(body, /<input[^>]*size="40"/)
    assert.match(body, /<input[^>]*value="7"/)
    assert.match(body, /<input[^>]*placeholder="Your player name"[^>]*value="7"/)
    assert.match(body, /<input[^>]*aria-describedby="data-form-field-1-after"/)
    const html = body.replace(/<!--.*?-->/g, "")
    assert.match(
      html,
      /<input[^>]*value="7"[^>]*\/>\s*<small id="data-form-field-1-after">Optional "cover" image\. Leave blank for no image\. &lt;unsafe(?:&gt;|>)<\/small>/
    )
    assert.doesNotMatch(
      body,
      /data-form-field-1-hint|<unsafe>|<small[^>]*>Your player name<\/small>/
    )
    assert.match(
      body,
      /<textarea[^>]*cols="80"[^>]*placeholder="Enter wiki source"[^>]*rows="8"[^>]*>\*\*hello\*\*<\/textarea>/
    )
    assert.match(body, /<textarea[^>]*aria-describedby="data-form-field-2-after"/)
    assert.match(
      html,
      /<textarea[^>]*>\*\*hello\*\*<\/textarea>\s*<small id="data-form-field-2-after">Plain source only<\/small>/
    )
    assert.match(body, /<fieldset[^>]*aria-describedby="data-form-field-3-after"/)
    assert.match(
      html,
      /<\/fieldset>\s*<small id="data-form-field-3-after">Choose one<\/small>/
    )
    assert.doesNotMatch(body, /data-form-field-3-hint|<small[^>]*>Choose rank<\/small>/)
    assert.match(body, /<textarea[^>]*cols="80"[^>]*rows="3"[^>]*>A summary<\/textarea>/)
    assert.match(body, /<input[^>]*id="data-form-field-10"[^>]*type="text"/)
    assert.doesNotMatch(
      body,
      /<textarea[^>]*id="data-form-field-10"|data-form-field-10-after/
    )
    assert.match(body, /<input[^>]*id="data-form-field-11"[^>]*type="text"/)
    assert.doesNotMatch(body, /Unsupported alias|data-form-field-11-hint/)
    assert.match(body, /<legend[^>]*>Rank<\/legend>/)
    assert.match(
      body,
      /<input[^>]*type="radio"[^>]*checked[^>]*\/>\s*String one<\/label>/
    )
    assert.match(body, /<legend[^>]*>Sex<\/legend>/)
    assert.match(body, /<input[^>]*type="radio"[^>]*checked[^>]*\/>\s*Female<\/label>/)
    assert.match(body, /<select[^>]*id="data-form-field-5"/)
    assert.match(body, /<option[^>]*selected[^>]*>Orc<\/option>/)
    assert.doesNotMatch(body, /unknown|name="wikitext"/)
    assert.equal(draft.rank, "1")
    assert.equal(draft.sex, false)

    const numeric = { ...form, values: { ...form.values, rank: 1, sex: true } }
    const numericBody = render(Component, {
      props: { form: numeric, draft: createDraft(numeric) }
    }).body
    assert.match(
      numericBody,
      /<input[^>]*type="radio"[^>]*checked[^>]*\/>\s*One<\/label>/
    )
    assert.doesNotMatch(
      numericBody,
      /<input[^>]*type="radio"[^>]*checked[^>]*\/>\s*String one<\/label>/
    )
    assert.match(
      numericBody,
      /<input[^>]*type="radio"[^>]*checked[^>]*\/>\s*Male<\/label>/
    )

    const nullable = { ...form, values: { ...form.values, rank: null } }
    const nullBody = render(Component, {
      props: { form: nullable, draft: createDraft(nullable) }
    }).body
    assert.match(nullBody, /<input[^>]*type="radio"[^>]*checked[^>]*\/>\s*Null<\/label>/)
    assert.doesNotMatch(
      nullBody,
      /<input[^>]*type="radio"[^>]*checked[^>]*\/>\s*String null<\/label>/
    )

    const unknown = { ...form, values: { ...form.values, rank: 99, sex: null } }
    const unknownDraft = createDraft(unknown)
    const unknownBody = render(Component, {
      props: { form: unknown, draft: unknownDraft }
    }).body
    assert.match(unknownBody, /Current value: 99/)
    assert.match(unknownBody, /Current value: null/)
    assert.doesNotMatch(unknownBody, /<input[^>]*type="radio"[^>]*checked/)
    assert.equal(unknownDraft.rank, 99)
    assert.equal(unknownDraft.sex, null)
    assert.deepEqual(changedFields(unknown, unknownDraft), {})

    const unknownRace = { ...form, values: { ...form.values, race: "legacy" } }
    const unknownRaceDraft = createDraft(unknownRace)
    const unknownRaceBody = render(Component, {
      props: { form: unknownRace, draft: unknownRaceDraft }
    }).body
    assert.match(unknownRaceBody, /<option[^>]*selected[^>]*>legacy<\/option>/)
    assert.deepEqual(changedFields(unknownRace, unknownRaceDraft), {})

    const missingValues = { ...form.values }
    delete missingValues.rank
    delete missingValues.sex
    const missing = { ...form, values: missingValues }
    const missingBody = render(Component, {
      props: { form: missing, draft: createDraft(missing) }
    }).body
    assert.doesNotMatch(missingBody, /<input[^>]*type="radio"[^>]*checked/)

    /** @type {import("../src/lib/form-editor").PageForm} */
    const unlabeled = {
      schema: {
        properties: {},
        fields: [
          {
            name: "null-text-unique",
            kind: "text",
            properties: { label: null },
            options: []
          },
          {
            name: "empty-wiki-unique",
            kind: "wiki",
            properties: { label: "" },
            options: []
          },
          {
            name: "missing-select-unique",
            kind: "select",
            properties: {},
            options: [
              { code: "a", label: "Alpha" },
              { code: "b", label: "Beta" },
              { code: "c", label: "Gamma" },
              { code: "d", label: "Delta" },
              { code: "e", label: "Epsilon" }
            ]
          },
          {
            name: "null-radio-unique",
            kind: "select",
            properties: { label: null, after: "Radio description" },
            options: [
              { code: "yes", label: "Yes" },
              { code: "no", label: "No" }
            ]
          },
          {
            name: 'unsafe<&"-unique',
            kind: "text",
            properties: { label: null },
            options: []
          },
          {
            name: "explicit-unique",
            kind: "text",
            properties: { label: "Visible <& label" },
            options: []
          }
        ]
      },
      values: {}
    }
    const unlabeledBody = render(Component, {
      props: { form: unlabeled, draft: createDraft(unlabeled) }
    }).body
    for (const name of [
      "null-text-unique",
      "empty-wiki-unique",
      "missing-select-unique",
      "null-radio-unique"
    ]) {
      assert.doesNotMatch(
        unlabeledBody,
        new RegExp(`<label[^>]*>${name}<\\/label>|<legend[^>]*>${name}<\\/legend>`)
      )
      assert.match(unlabeledBody, new RegExp(`aria-label="${name}"`))
    }
    assert.doesNotMatch(
      unlabeledBody,
      /<label for="data-form-field-[0124]"|<legend[^>]*>null-radio-unique<\/legend>/
    )
    assert.match(
      unlabeledBody,
      /<input[^>]*id="data-form-field-0"[^>]*aria-label="null-text-unique"/
    )
    assert.match(
      unlabeledBody,
      /<textarea[^>]*id="data-form-field-1"[^>]*aria-label="empty-wiki-unique"/
    )
    assert.match(
      unlabeledBody,
      /<select[^>]*id="data-form-field-2"[^>]*aria-label="missing-select-unique"/
    )
    const radioTag = /<fieldset\b[^>]*>/.exec(unlabeledBody)?.[0]
    assert.ok(radioTag, "unlabeled radio group must render")
    assert.match(radioTag, /aria-label="null-radio-unique"/)
    assert.match(radioTag, /aria-describedby="data-form-field-3-after"/)
    assert.match(
      unlabeledBody,
      /<small id="data-form-field-3-after">Radio description<\/small>/
    )
    assert.match(
      unlabeledBody,
      /<label class="radio-option(?:\s[^"]*)?"[^>]*><input[^>]*\/> Yes<\/label>/
    )
    assert.match(unlabeledBody, /aria-label="unsafe&lt;&amp;&quot;-unique"/)
    assert.doesNotMatch(unlabeledBody, /<label for="data-form-field-4">|<unsafe/)
    assert.match(
      unlabeledBody,
      /<label for="data-form-field-5">Visible &lt;&amp; label<\/label>/
    )
    assert.doesNotMatch(unlabeledBody, /aria-label="explicit-unique"/)
  } finally {
    await unlink(fixture)
  }
})
