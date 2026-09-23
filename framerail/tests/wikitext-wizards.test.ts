import assert from "node:assert/strict"
import { test } from "node:test"
import {
  applyWizard,
  extractEquations,
  normalizeFlickrSource
} from "../src/lib/wikitext-wizards.ts"

test("table inserts at selection start without consuming selected text", () => {
  assert.deepEqual(
    applyWizard("intro selected tail", 6, 14, {
      kind: "table",
      rows: 2,
      columns: 2,
      headers: true
    }),
    {
      value:
        "intro \n\n||~ header ||~ header ||\n|| cell-content || cell-content ||\n\nselected tail",
      start: 67,
      end: 67
    }
  )
})

test("table at document start and end preserves source newline rules", () => {
  assert.deepEqual(
    applyWizard("", 0, 0, {
      kind: "table",
      rows: 1,
      columns: 1,
      headers: false
    }),
    { value: "\n\n|| cell-content ||", start: 20, end: 20 }
  )
  assert.deepEqual(
    applyWizard("abc\n", 4, 4, {
      kind: "table",
      rows: 1,
      columns: 1,
      headers: false
    }),
    { value: "abc\n\n|| cell-content ||", start: 23, end: 23 }
  )
})

test("code wraps trimmed selection with blank lines and collapses caret", () => {
  const value = "before \n  echo  \nafter"
  const result = applyWizard(value, 9, 15, { kind: "code", type: "PHP" })
  assert.deepEqual(result, {
    value: 'before \n  \n\n[[code type="PHP"]]\necho\n[[/code]]\n\n  \nafter',
    start: 46,
    end: 46
  })
})

test("code without selection selects placeholder", () => {
  const result = applyWizard("", 0, 0, { kind: "code", type: "" })
  assert.deepEqual(result, {
    value: "[[code]]\ninsert the code here\n[[/code]]",
    start: 9,
    end: 29
  })
})

test("URI renders anchor and new-window marker at captured start", () => {
  assert.deepEqual(
    applyWizard("abcXYZ", 3, 6, {
      kind: "uri",
      uri: "https://example.org",
      anchor: "visit",
      newWindow: true
    }),
    {
      value: "abc[*https://example.org visit]XYZ",
      start: 31,
      end: 31
    }
  )
  assert.deepEqual(
    applyWizard("x", 0, 1, {
      kind: "uri",
      uri: "ftp://example.org/a",
      anchor: "",
      newWindow: true
    }),
    {
      value: "*ftp://example.org/a" + "x",
      start: 20,
      end: 20
    }
  )
})

test("page link inserts literal target and optional anchor", () => {
  assert.deepEqual(
    applyWizard("abc", 1, 3, {
      kind: "pageLink",
      page: "category:page",
      anchor: "Read more"
    }),
    {
      value: "a[[[category:page |Read more]]]bc",
      start: 31,
      end: 31
    }
  )
})

test("image preserves position conversion and optional size", () => {
  assert.deepEqual(
    applyWizard("tail", 0, 4, {
      kind: "image",
      source: "file",
      value: "logo.png",
      position: "fl",
      size: "small"
    }),
    {
      value: '[[f<image logo.png size="small"]]tail',
      start: 33,
      end: 33
    }
  )
  assert.equal(
    applyWizard("", 0, 0, {
      kind: "image",
      source: "uri",
      value: "https://example.org/a.png",
      position: "c"
    }).value,
    "[[=image https://example.org/a.png]]"
  )
})

test("Flickr normalization accepts photo page, numeric ID and static image URL", () => {
  assert.equal(normalizeFlickrSource("12345"), "flickr:12345")
  assert.equal(
    normalizeFlickrSource("https://www.flickr.com/photos/user/12345/in/album"),
    "flickr:12345"
  )
  assert.equal(
    normalizeFlickrSource("http://static.flickr.com/12/12345_abc2_m.jpg"),
    "flickr:12345_abc2"
  )
  assert.equal(
    applyWizard("", 0, 0, {
      kind: "image",
      source: "flickr",
      value: "12345",
      position: "fr"
    }).value,
    "[[f>image flickr:12345]]"
  )
})

test("equations include only complete line-start alphanumeric-labelled blocks", () => {
  assert.deepEqual(
    extractEquations(
      "[[math A1]]\nx+y\n[[/math]]\n [[math indented]]\na\n[[/math]]\n[[math bad-name]]\na\n[[/math]]\n[[math B2]]\r\nq\r\n[[/math]]"
    ),
    [
      { label: "A1", source: "x+y" },
      { label: "B2", source: "q" }
    ]
  )
})

test("equation reference only uses a discovered label", () => {
  const value = "[[math A1]]\nx+y\n[[/math]]\ntail"
  const at = value.indexOf("tail")
  assert.deepEqual(
    applyWizard(value, at, value.length, {
      kind: "eref",
      label: "A1",
      withEq: true
    }),
    {
      value: "[[math A1]]\nx+y\n[[/math]]\nEq.([[eref A1]])tail",
      start: at + 16,
      end: at + 16
    }
  )
  assert.equal(
    applyWizard(value, at, at, {
      kind: "eref",
      label: "A1",
      withEq: false
    }).value,
    "[[math A1]]\nx+y\n[[/math]]\n[[eref A1]]tail"
  )
  assert.throws(
    () =>
      applyWizard(value, at, at, {
        kind: "eref",
        label: "missing",
        withEq: false
      }),
    /Equation label not found/
  )
})

test("invalid dimensions, options and malformed selection fail explicitly", () => {
  for (const rows of [0, 100, 1.5, NaN]) {
    assert.throws(
      () =>
        applyWizard("", 0, 0, {
          kind: "table",
          rows,
          columns: 2,
          headers: false
        }),
      /rows.*1\.\.99/
    )
  }
  assert.throws(
    () =>
      applyWizard("", 0, 0, {
        kind: "table",
        rows: 1,
        columns: 0,
        headers: false
      }),
    /columns.*1\.\.99/
  )
  assert.throws(
    () =>
      applyWizard("x", -1, 0, {
        kind: "code",
        type: ""
      }),
    RangeError
  )
  assert.throws(
    () =>
      applyWizard("", 0, 0, {
        kind: "code",
        type: 'PHP" evil="yes'
      }),
    /code type/
  )
  assert.throws(
    () =>
      applyWizard("", 0, 0, {
        kind: "uri",
        uri: "https://example.org/a b",
        anchor: "",
        newWindow: false
      }),
    /URI/
  )
  assert.throws(
    () =>
      applyWizard("", 0, 0, {
        kind: "pageLink",
        page: "x]]]",
        anchor: ""
      }),
    /page/
  )
  assert.throws(
    () =>
      applyWizard("", 0, 0, {
        kind: "image",
        source: "file",
        value: "x]]",
        position: ""
      }),
    /image/
  )
  assert.throws(
    () =>
      applyWizard("", 0, 0, {
        kind: "image",
        source: "uri",
        value: "a.png",
        position: "",
        size: 's"]]'
      }),
    /image size/
  )
  assert.throws(() => normalizeFlickrSource("https://evil.example/photos/123"), /Flickr/)
})
