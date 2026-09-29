# Image delivery errors: replica versus Wikidot

verified: 2026-09-29 UTC

Follow-up to the [production crawl](cobalt-page-performance-crawl.md), limited to its **56 image 5xx failures**: 53 pet icons and three character icons. This is not the separate 404/malformed-image-URL cohort. Investigation only; no fixes, deployment, cache purge, permission changes or burst replay.

## Were the original images missing?

**All 56 original full-size files were retrieved successfully and are byte-for-byte identical to the replica copies.** SHA-256 was recomputed from downloaded bodies, not inferred from matching filenames or HTTP 200 alone. All replica responses were valid JPEGs. The original `icons` and `pet-icons` HTML referenced every affected filename.

Wikidot's pages display resized `thumbnail.jpg` URLs, whereas the underlying originals are separate `local--files` URLs. Both representations were checked:

| Check | First bounded HTTP batch, after retries | Follow-up result |
|---|---:|---:|
| Original full-size files | 51 × 200; 5 × 500 | All 56 retrieved; all 56 match replica bytes |
| Original displayed thumbnails | 49 × 200; 7 × 500 | 55 retrieved; one still returned 500 in a browser recheck |
| Replica full-size files | 56 × 200 | All match verified originals |

The remaining failing source thumbnail was `pet-icon_raptor-blue.jpg/thumbnail.jpg`. Its full original and replica copy both work and match. That is a source thumbnail/delivery failure, **not proof that the underlying image is missing**.

Source errors returned a short Nginx 500 HTML page, not a 404 or an image. Standard-browser rechecks with the referring Wikidot page recovered the five originals and six thumbnails. The checks did not isolate elapsed-time effects from request-header/client differences. They also cannot reconstruct Wikidot availability at the exact time of the earlier replica incident.

## What failed on the replica?

| Browser failures | Correlated evidence | Supported conclusion |
|---|---|---|
| 53 pet-icon 502s | Every exact filename appears in Cloudflare Tunnel logs with EOF or connection-reset errors reading `127.0.0.1:3088`. | Tunnel-to-local-gateway delivery failed. These were not missing-source-file responses. |
| `icon_amerygn.jpg`, `icon_lanlu.jpg`: two 502s | Exact filenames have tunnel-to-gateway connection resets at 06:02:46 UTC. | Same failing transport boundary as the pet-icon group. |
| `icon_gruze.jpg`: one 500 | WWS logged its file-metadata RPC being rejected with HTTP 429 at 06:02:46.484 UTC; WWS maps file-fetch errors to 500. | The metadata request failed before normal image delivery. |

All **55 browser 502 URLs** have a corresponding tunnel-origin read failure in the incident window. Cloudflare producing the visible 502 page is consistent with these logs, but the original 502 bodies/headers were not retained; their exact HTTP-body producer is not proven.

Two capacity constraints are evidenced:

- Nginx logged `256 worker_connections are not enough` during roster/class loading. Connections to upstreams consume this budget too; it is not simply a count of browser tabs.
- Deepwell uses `jsonrpsee-server` 0.26.0 without overriding its **100-connection default**. That library returns HTTP 429 when no connection permit remains. WWS connects directly to Deepwell's loopback port. This is connection admission/backpressure, not an established per-second rate limit.

The exact low-level cause of each gateway EOF/reset remains unproven. In particular, the earlier Nginx alerts do not by themselves explain the later pet-icon burst. No new burst was generated to force the failure.

## Ruled-out and unproven explanations

- **Missing imports:** not supported for this cohort; all 56 replica copies match original full-file bytes.
- **A live dependency on Wikidot image delivery:** WWS serves the replica's stored S3 blobs, not a fresh Wikidot download per image request. Source-side thumbnail errors therefore do not explain the observed localhost tunnel/RPC failures.
- **Double blob downloads in metadata lookup:** `file_get` uses `data: false`; `BlobService::get_maybe(false, ...)` returns `None` without fetching a blob.
- **S3 timeout:** deployed WWS source has a 200 ms S3 request timeout, but the retained cohort logs do not establish it as the cause of these 56 events. Do not promote it from a candidate to a finding.

## Provenance and method

Live WWS executable SHA-256 `6ca1dfe2d42f78344812fe88da07fc3afee420d4b96d2006d34c9484eecc5d59` matches the local Nix output. The local retained deriver resolves its actual source to `/nix/store/w5ci8plnhhf5b69c6ki3hval97xn9aib-source`; this confirmed the deployed metadata and timeout paths, rather than assuming current checkout code was deployed.

Source and replica HTTP comparisons were sequential, limited to one request start per second, with bounded transient retries. Image MIME/signatures were checked and bodies retained. Browser follow-ups navigated only the failed source URLs sequentially; no credentials or source-site forms were used. No 404s occurred in the original-file/thumbnail comparison probes.

Implementation references:

- `deepwell/src/api.rs` and locked `jsonrpsee-server` 0.26.0 — server connection configuration.
- `wws/src/state.rs`, `wws/src/deepwell.rs`, `wws/src/error/basic.rs` — metadata/permission RPC errors and file-fetch status mapping.
- `wws/src/handler/file.rs`, `wws/src/fetch.rs` — stored-object delivery.
- `deepwell/src/services/blob/service.rs` — conditional blob fetching.
- [jsonrpsee connection configuration](https://docs.rs/jsonrpsee-server/0.26.0/jsonrpsee_server/struct.ServerConfigBuilder.html#method.max_connections) and [Nginx connection accounting](https://nginx.org/en/docs/ngx_core_module.html#worker_connections) — primary documentation.

## Evidence

Protected comparison directory:

`/home/osso/.local/share/cobalt-wiki/image-investigation/20260929T064943Z/`

- `comparison.jsonl`, source/replica response headers and bodies — initial attempts, including source failures.
- `source-browser-rechecks.json`, `source-original-recovery-results.json`, `recovered-source-*.body` — browser follow-ups and recovered original bytes.
- `comparison-final.csv`, `comparison-final.json`, `final-summary.json` — per-file outcome and SHA-256 comparison.
- `cloudflared-incident.log`, `tunnel-file-correlation.json`, `incident-logs/` — filename/timestamp correlation and WWS/gateway evidence.
- `deployed-wws-provenance.json`, `wws-build-source-path.txt`, `wws-runtime-sanitized.json` — running artifact, actual source and loopback endpoint.
