import { JSONRPCErrorException } from "json-rpc-2.0"

/**
 * Keep unexpected exceptions on the framework's error path, not form
 * failures.
 */
export function requireDeepwellError(error: unknown): {
  message: string
  code: number
  data: unknown
} {
  if (!(error instanceof JSONRPCErrorException)) throw error
  return { message: error.message, code: error.code, data: error.data }
}

/**
 * RPC data is arbitrary JSON; only these two shapes contain displayable
 * details.
 */
export function errorDetails(data: unknown): string | undefined {
  if (typeof data === "string") return data
  if (
    typeof data === "object" &&
    data !== null &&
    "call_trace" in data &&
    typeof data.call_trace === "string"
  ) {
    return data.call_trace
  }
}
