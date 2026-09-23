# Cobalt queue incident and recovery

Verified: 2026-09-23. This records the September 22–23 queue incident, containment, rollback evidence, deployed attachment fix, and post-recovery state. It does not claim rendering parity or history migration.

## Cause and fix

First attachment revisions used page-displacement invalidation. An attachment owner's ordinary linkers, including navigation pages, could therefore receive `Full` work; navigation fanout made this site-wide. The first revision now uses page-edit invalidation. Commit `b007c0b` was verified in production order with a real upload: ordinary owner-page links enqueue no rerender, while a navigation-page owner retains necessary navigation fanout.

The isolated hotfix `f18dcfed691983a12021d6c1da372203ce145138`, based on `474b308be897f40d96767c585e96c45fa3c55095`, is deployed. Host commit `72bd4a4` pins it. Independent deployment confirmation found remote generation `b8lyk2lnn46pkgw1w2l9nqyy9bcy5qy8-nixos-system-sakuin-digital-ocean-26.05.20260606.9b69646`; the source diff is exactly three files, with one runtime line changing `process_page_displace` to `process_page_edit`. `ops/deploy.sh` ended with `Ready. Deploy complete.` User-owned rendering work was not included or touched.

See [attachment invalidation](../../specs/cobalt-attachment-invalidation.md) for the behavioral contract.

## Recovery

- Immutable pre-maintenance RDB backup: `/var/lib/cobalt-wiki/queue-backup-20260922T225209Z/cache.rdb`; SHA-256 `33cd8c7321b3d33261c9357e03dff6b9daef6e6fa18e6eadc1d31e814bcf05b2`.
- The first guarded attempt stalled at the Cobalt slice's 2 GiB cap. It exceeded the original ten-minute limit by 15 seconds; the cumulative outage was 615 seconds. The user then explicitly set no SLA and no per-maintenance approval requirement.
- Final recovery paused all non-cache Cobalt processes, processed guarded batches of 512 records, and wrote durable JSONL rollback journals. It preserved every non-duplicate job type.
- Previously received full/navigation retry jobs were preserved. They created a smaller second wave, which completed; tail cleanup recorded at `/var/lib/cobalt-wiki/queue-recovery-stage-20260922/run-tail-20260923T003235Z/result.json` removed duplicate rerenders again.

## Post-recovery proof

`/home/osso/.local/share/cobalt-wiki/queue-cleared-proof.json` records only four maintenance jobs and zero rerenders. The four remaining jobs are maintenance work. Valkey allocated memory was `1,855,480` bytes and RSS was `42,729,472` bytes; AOF auto-fork percentage returned to 100.

`/home/osso/.local/share/cobalt-wiki/post-queue-data-proof.json` records no source or file inventory changes (6,092 sources, 1,471 files) and matches the five largest byte hashes. All Cobalt and Sakuin services were active after deployment. The authenticated origin returned 200 with no-index; protected gateway checks returned 401 unauthenticated and 200 authenticated with no-index. The public browser check returned 200. Sakuin readiness returned 200 with an unchanged PID.

Original queue counts and candidate evidence remain protected in the recovery artifacts; this page intentionally does not repeat them.

## Boundaries

The recovery removed only proven duplicate pending navigation rerenders. It did not delete source, pages, files, history, received jobs, full jobs, maintenance jobs, malformed jobs, or other job types. The user-owned rendering branch was untouched.
