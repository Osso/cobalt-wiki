import assert from "node:assert/strict"
import { test } from "node:test"
import { JSONRPCClient, JSONRPCErrorException } from "json-rpc-2.0"
import { createJiti } from "jiti"
import { fail } from "@sveltejs/kit"

const { requireDeepwellError, errorDetails } = await createJiti(import.meta.url).import<
  typeof import("../src/lib/deepwell-errors")
>("../src/lib/deepwell-errors")

test("actual RPC rejection retains the validation message, code and payload", async () => {
  const client = new JSONRPCClient(async (request) => {
    client.receive({
      jsonrpc: "2.0",
      id: request.id!,
      error: {
        code: -32000,
        message: "Title is required",
        data: { call_trace: "page_edit", field: "title" }
      }
    })
  })
  await assert.rejects(client.request("page_edit", {}), (error: unknown) => {
    assert.deepEqual(requireDeepwellError(error), {
      message: "Title is required",
      code: -32000,
      data: { call_trace: "page_edit", field: "title" }
    })
    return true
  })
})

test("non-RPC thrown values are rethrown unchanged", () => {
  for (const value of [
    new Error("network"),
    { message: "fake", code: 4 },
    null,
    "failure"
  ]) {
    try {
      requireDeepwellError(value)
      assert.fail("must throw")
    } catch (error) {
      assert.equal(error, value)
    }
  }
})

test("RPC errors without data retain their original message and code", () => {
  assert.deepEqual(requireDeepwellError(new JSONRPCErrorException("Denied", 403)), {
    message: "Denied",
    code: 403,
    data: undefined
  })
})

test("action failures preserve status, form validation state and RPC payload", () => {
  const form = { valid: false, errors: { title: ["Title is required"] } }
  for (const status of [400, 500]) {
    const failure = fail(status, {
      form,
      ...requireDeepwellError(
        new JSONRPCErrorException("Validation failed", -32000, { field: "title" })
      )
    })
    assert.equal(failure.status, status)
    assert.equal(failure.data.form, form)
    assert.deepEqual(failure.data, {
      form,
      message: "Validation failed",
      code: -32000,
      data: { field: "title" }
    })
  }
})

test("popup displays only string details or a string call trace", () => {
  assert.equal(errorDetails("Validation failed"), "Validation failed")
  assert.equal(errorDetails({ call_trace: "page_edit" }), "page_edit")
  for (const data of [null, 3, [], {}, { call_trace: 3 }])
    assert.equal(errorDetails(data), undefined)
})
