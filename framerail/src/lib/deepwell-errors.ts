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
 * HTTP status for a Deepwell error code, by the ranges documented in
 * `deepwell/src/error/error_type.rs`. Unknown and server-side codes are
 * 500.
 */
export function deepwellErrorStatus(code: number): number {
  if (code >= 2000 && code < 2100) return 404
  if (code >= 2100 && code < 2200) return 409
  if (code >= 3000 && code < 3100) return 401
  if (code >= 3100 && code < 3200) return 403
  if (code >= 4000 && code < 7000) return 400
  return 500
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
