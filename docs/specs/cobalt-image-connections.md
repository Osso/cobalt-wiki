# Cobalt image connection capacity

Ordinary image-heavy pages must fit the gateway and internal RPC admission budgets without resetting clients. The [incident investigation](../wiki/systems/cobalt-image-delivery-investigation.md) traced 55 tunnel-origin failures and one metadata RPC rejection; this change addresses locally reproduced connection exhaustion without changing file access or freshness.

## What it must do

- [x] The generated gateway handles 350 concurrent held image responses plus 64 idle client keep-alives, preserving every response body and reusable client connection.
- [x] A lower active burst (80 proxied requests alongside 180 idle clients) does not forcibly reclaim those idle connections within the supported budget.
- [x] Gateway forwarding still supplies the trusted site ID and removes client-supplied Authorization and user-ID headers.
- [x] Deepwell admits 512 concurrent active HTTP RPC calls, rejects an additional active call with 429, and accepts calls again after capacity is released. Idle HTTP keep-alives do not consume this request-admission budget.
- [ ] Owning production configuration and app deployment apply these settings; ordinary roster and pet-icon browser checks show no connection-related image 5xx in the bounded post-deploy sample.

## How it works

- [Image incident and source comparison](../wiki/systems/cobalt-image-delivery-investigation.md)
- [Roster loading profile](../wiki/systems/cobalt-roster-performance.md)

## Implementation inventory

- `install/nixos/poc-gateway.nix` — 1,024 Nginx connection slots. Each proxied request needs client and upstream slots; roughly 400 resource requests plus retained clients fit this budget. Gateway soft `LimitNOFILE` rises from 1,024 to 2,048 for socket/log/temp-file overhead; its existing 524,288 hard limit is preserved.
- `deepwell/src/api.rs` — shared production/test transport builder with a bounded 512 admission limit, giving WWS's 256 in-flight RPCs room alongside other internal clients. Database pool and shared memory caps remain unchanged.
- `tests/gateway/test_connection_capacity.py` — real Nginx 1.30.2 and a loopback-only held-response fixture, using configuration generated from the actual Nix module. Only ports, runtime paths and diagnostic logging are adjusted for isolation.
- `deepwell/src/api/tests.rs` — real HTTP/jsonrpsee admission, idle-lifetime and capacity-recovery tests.

## Tests asserting this spec

- Gateway RED at 256 slots: the 350-image case returned 119 complete responses and 231 transport errors; all 64 idle sockets closed. The 80-active/180-idle case served all active responses but reclaimed 104 idle sockets, with one recycling warning and **no hard-limit alert**. This reproduces why EOFs need not have a matching alert-level log.
- Gateway GREEN at 1,024 slots: both cases pass, all 350/80 active responses and 64/180 reused idle clients succeed; no recycling or transport errors. `/tmp/cobalt-gateway-capacity-{red2,green}.log`.
- RPC RED with the library default: requests rejected before the held-call target. GREEN at 512: the limit remains enforced, recovers after release, and 512 idle HTTP keep-alives still permit a new RPC. `/tmp/cobalt-rpc-capacity-proof.md`.

## Known gaps (current cycle)

- [ ] Complete owning host/app rollout and final independent/post-deploy proof.
- [ ] These finite budgets do not guarantee arbitrary concurrent traffic. The isolated reproduction establishes the connection-reclamation failure mechanism, not the exact state of every historical socket.

## Out of scope

Changing image formats/loading, file permission checks, cache-control/ETag policy, S3 timeouts, database connection limits, missing-file/parser errors, or source Wikidot thumbnail failures.
