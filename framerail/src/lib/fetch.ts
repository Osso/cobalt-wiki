// Wrapper around fetch() to provide timeouts.

export const DEFAULT_TIMEOUT = 1500

export function wjfetch(
  url: RequestInfo | URL,
  options: RequestInit & { timeout?: number } = {}
) {
  let timeout = DEFAULT_TIMEOUT
  if (options.timeout) {
    timeout = options.timeout
    delete options.timeout
  }

  return fetch(url, { signal: AbortSignal.timeout(timeout), ...options })
}
