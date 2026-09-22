import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { test } from "node:test"
import { runInNewContext } from "node:vm"

const source = readFileSync(
  new URL("../src/lib/vendor/request-idle-callback-polyfill.js", import.meta.url),
  "utf8"
)

function scheduler() {
  let now = 0
  let nextId = 0
  /** @type {Map<number, { at: number; callback: () => void }>} */
  const timers = new Map()
  const host = {
    performance: { now: () => now },
    /** @param {() => void} callback @param {number} [delay] */
    setTimeout(callback, delay = 0) {
      const id = ++nextId
      timers.set(id, { at: now + delay, callback })
      return id
    },
    /** @param {number} id */
    clearTimeout(id) {
      timers.delete(id)
    }
  }
  runInNewContext(source, host)
  const shim =
    /**
     * @type {typeof host & {
     *   idleCallbackShim: {
     *     request: (callback: IdleRequestCallback) => number
     *     cancel: (id: number) => void
     *   }
     * }}
     */ (host).idleCallbackShim
  return {
    shim,
    /** @param {number} milliseconds */
    advance(milliseconds) {
      const end = now + milliseconds
      for (;;) {
        const next = [...timers].sort((a, b) => a[1].at - b[1].at)[0]
        if (!next || next[1].at > end) break
        now = next[1].at
        timers.delete(next[0])
        next[1].callback()
      }
      now = end
    },
    /** @param {number} milliseconds */
    consume(milliseconds) {
      now += milliseconds
    }
  }
}

test("queued callbacks run after throttle and cancellation preserves later requests", () => {
  const clock = scheduler()
  /** @type {number[]} */
  const calls = []
  const first = clock.shim.request(() => calls.push(1))
  const cancelled = clock.shim.request(() => calls.push(2))
  const third = clock.shim.request(() => calls.push(3))
  assert.deepEqual([first, cancelled, third], [1, 2, 3])
  clock.shim.cancel(cancelled)
  clock.advance(124)
  assert.deepEqual(calls, [])
  clock.advance(1)
  assert.deepEqual(calls, [1, 3])
  const fourth = clock.shim.request(() => calls.push(4))
  clock.shim.cancel(first)
  clock.advance(125)
  assert.equal(fourth, 4)
  assert.deepEqual(calls, [1, 3, 4])
})

test("deadlines decrease with elapsed work and exhausted work defers the next task", () => {
  const clock = scheduler()
  /** @type {number[]} */
  const remaining = []
  let secondRan = false
  clock.shim.request((deadline) => {
    assert.equal(deadline.didTimeout, false)
    remaining.push(deadline.timeRemaining())
    clock.consume(3)
    remaining.push(deadline.timeRemaining())
    clock.consume(10)
    remaining.push(deadline.timeRemaining())
  })
  clock.shim.request(() => {
    secondRan = true
  })
  clock.advance(125)
  assert.deepEqual(remaining, [7, 4, 0])
  assert.equal(secondRan, false)
  clock.advance(125)
  assert.equal(secondRan, true)
})

test("existing modern idle callbacks remain installed", () => {
  /**
   * @param {IdleRequestCallback} _callback @param {IdleRequestOptions}
   *   [_options]
   */
  const requestIdleCallback = (_callback, _options) => 42
  /** @param {number} _id */
  const cancelIdleCallback = (_id) => {}
  const host = { requestIdleCallback, cancelIdleCallback, setTimeout, clearTimeout }
  runInNewContext(source, host)
  assert.equal(host.requestIdleCallback, requestIdleCallback)
  assert.equal(host.cancelIdleCallback, cancelIdleCallback)
})

test("legacy numeric timeout and deadline getter remain compatible", () => {
  /** @type {(number | undefined)[]} */
  const timeouts = []
  const prototype = {}
  Object.defineProperty(prototype, "timeRemaining", {
    get() {
      return 12
    },
    configurable: true
  })
  const host = {
    setTimeout,
    clearTimeout,
    IdleCallbackDeadline: { prototype },
    /**
     * @param {IdleRequestCallback} _callback @param {number |
     *   IdleRequestOptions} [timeout]
     */
    requestIdleCallback(_callback, timeout) {
      if (typeof timeout === "object") throw new TypeError("numeric timeout required")
      timeouts.push(timeout)
      return 9
    },
    /** @param {number} _id */
    cancelIdleCallback(_id) {}
  }
  runInNewContext(source, host)
  assert.equal(
    host.requestIdleCallback(() => {}, { timeout: 17 }),
    9
  )
  host.requestIdleCallback(() => {})
  assert.deepEqual(timeouts, [17, undefined])
  const deadline = Object.create(prototype)
  assert.equal(deadline.timeRemaining(), 12)
})
