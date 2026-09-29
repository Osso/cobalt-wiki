# Roster loading profile

verified: 2026-09-29 UTC

Production `cobalt-company.sakuin.org`, app release `ac963e66eb46`; see [tag rollout](../../specs/cobalt-tag-admin.md#production-rollout). Read-only investigation, ordinary anonymous page visits, no optimization changes or broad rerender sweep. Small samples from one desktop Chromium instance, not percentiles or a load test.

## Observations

| Layer | Measured result | Interpretation |
|---|---|---|
| Race roster after deploy | HTML TTFB 1,046 ms; next visit 267 ms. Class roster 658 → 124 ms. | First-view penalty consistent with synchronous build-mismatch rerender. Before the first visit, race/class stored `f77f6976e79a`; race afterward stored `ac963e66eb46`. Roster itself was warmed by deploy smoke. Other cold work/network variability is not isolated. |
| Warm renderer/view | Host-local `page_view` race 11–13 ms, roster 13–15 ms; `preload_view` 3.3–3.5 ms. Responses checked as successful. | Cached HTML lookup is not taking seconds. |
| Warm full HTML origin | Race via Framerail 51–55 ms to first byte; via gateway 55–67 ms. Public browser warm TTFB 247–289 ms. | Remaining public latency includes network/proxy work; it is not all rendering. |
| Listing SELECT | 6,110 live pages, exact site metadata query 29.6 ms, 1,890 shared-buffer hits. | Broad query exists, but this sample does not justify making a DB index/query rewrite the first optimization. |
| Cold browser, warm backend | Race 4.137 s load, 2.264 s logo LCP; repeat 1.976 s load, 328 ms logo LCP. | The image/asset tail persists independently of cold renderer work. Full-load time is not the same as time to useful content. |
| Image traffic | 306 image requests; 7.06 MB image bytes out of 7.37 MB total on first visit. Repeat still 306 requests: 300×304, 3×200, 3×404; 172 KB image traffic. | Reusing cached bodies reduces bandwidth, not the request fan-out. All DOM images use `loading=auto`. |
| Gateway capacity | Nginx logged `256 worker_connections are not enough` at 02:14:23 UTC during the initial navigation sequence. Config sets `worker_connections 256`. | A real capacity limit was reached; the fraction of page delay attributable to it was not separately measured. |
| Browser CPU | Race 2,707 DOM nodes/302 image elements; first run one 55 ms long task, subsequent runs none. | No evidence that sustained JavaScript/layout blocking is the dominant bottleneck. |

### Image details

- 300 portraits have natural dimensions 144×144 and display at 140×140: a missing thumbnail resizer is not the obvious first fix.
- Cold transfer: background 419 KB, logo 67 KB; larger portraits around 35–36 KB. Images account for roughly 96% of transferred bytes.
- Cold logo request: low priority, starts 379 ms, headers at 2,212 ms, ends 2,257 ms; logo LCP 2,264 ms. Network queueing and origin latency are not individually attributed by this sample.
- Repeat portrait responses have `Cache-Control: public, no-cache`, ETag and `CF-Cache-Status: EXPIRED`.
- Direct origin test with the current portrait ETag returned **200 plus all 12,943 bytes**, not 304, in 83 ms. The retained curl trace includes the outgoing `If-None-Match` and identical response ETag. WWS currently fetches current metadata/authorization, then the object; its file handler does not short-circuit matching `If-None-Match`. Edge/browser revalidation savings therefore do not prove the origin avoided its body work.
- Missing `icon_emdee.jpg` and `icon_valentine.jpg` account for the three image 404 requests. They are separate content/resource issues, not an established explanation for the general delay.

## Optimization choices — not implemented

1. **Reduce initial image fan-out.** Defer below-fold portraits and prioritize the above-fold logo. Targets hundreds of eager requests, initial bytes, and gateway pressure; preserve roster content and scrolling behavior.
2. **Make unchanged-image revalidation cheap.** After current authorization and metadata checks, honor matching ETags without reading/streaming S3 bodies. Preserve [replacement/access freshness](../../specs/cobalt-media-routing.md); do not replace it with a long stale TTL.
3. **Address gateway connection capacity.** Raise/tune the observed 256-connection ceiling within host resource limits. Reduces the known saturation risk but does not eliminate image bandwidth or per-image origin work.
4. **Avoid unnecessary cold rerenders.** Current app deployment uses the whole Git commit in renderer identity, so non-rendering changes can invalidate stored HTML. Use a correctly scoped identity or bounded critical-page warming, with explicit freshness coverage. Targets the observed first-view penalty, not every warm image load.

No speedup guarantee is inferred from these few measurements. User chooses optimization scope before implementation.

## Source and proof

- `install/dev-deploy.sh`; `deepwell/src/services/view/service.rs` — build identity and synchronous stale render.
- `deepwell/src/services/render/list_pages.rs` — one site metadata load, in-memory selection and form-value loading.
- `wws/src/handler/file.rs`, `wws/src/fetch.rs`, `wws/src/state.rs`, `wws/src/visibility.rs` — image body delivery, current authorization/metadata, ETag/cache policy.
- `install/nixos/poc-gateway.nix` — 256 worker connections and Framerail/WWS routing.
- `/tmp/cobalt-roster-inventory-before.csv`, `/tmp/cobalt-profile-roster-by-race-live.json` — before/after renderer identity. `compiled_at` was not updated by rerender; do not use its old timestamp to date the current build.
- `/tmp/cobalt-roster-browser-timings.json` — first postdeploy navigation sequence; its Resource Timing buffer hit 250 entries, so it is not the complete asset inventory.
- `/tmp/cobalt-roster-full-waterfall.json` — separate warm-backend/cold-browser then repeat samples, expanded Resource Timing buffer and CDP status/byte details.
- `/tmp/cobalt-roster-host-rpc-timings.json`, `/tmp/cobalt-roster-host-http-timings.json`, `/tmp/cobalt-roster-sql-profile.txt`, `/tmp/cobalt-roster-gateway-log.txt`, `/tmp/cobalt-roster-origin-conditional-verified.txt` — direct layer measurements and gateway alert.
