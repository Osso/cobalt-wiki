# Cobalt message reply/forward access

Message draft creation accepts `reply_to` and `forwarded_from` references only for the source message's sender or recipients. Source lives in `deepwell/src/services/message/`; see [message access architecture](../wiki/systems/cobalt-message-access.md) for implementation documentation.

## What it must do

- [x] Allow the source message's sender to create a draft referencing it through either `reply_to` or `forwarded_from`.
- [x] Allow regular, CC, and BCC recipients to create a draft through either reference; the returned draft must match the persisted row and existing drafts must remain unchanged.
- [x] Reject outsiders and missing source messages through either reference with `MessageNotFound` (code `2014`), preserving all existing owned draft rows without inserting a draft.

## How it works

- [Message access architecture](../wiki/systems/cobalt-message-access.md) (documentation stub).

## Implementation inventory

- `deepwell/src/endpoints/message.rs` — registered message draft creation and sending endpoints.
- `deepwell/src/services/message/service.rs` — source-message access validation and draft lifecycle.
- `deepwell/src/api/message_access_tests.rs` — reply/forward functional access matrix.

## Tests asserting this spec

`deepwell/src/api/message_access_tests.rs`: 12 actual registered HTTP JSON-RPC/PostgreSQL cases at code revision `25c536514f39510a8891b0860e5e56e011e19f43`. Source messages are created and sent through `message_draft_create` and `message_draft_send`; users are seeded in SQL. Sender covers two cases, regular/CC/BCC recipients six, outsider two, and missing source two.

Supplied proof ledger records eight expected RED failures and 12/12 GREEN passes, including persisted draft equality and full owned-draft snapshots on denial. Evidence: `/tmp/cobalt-security-message-proof-ledger.md` and `/tmp/cobalt-security-message-report.md`; this documentation update does not rerun tests.

## Known gaps (current cycle)

No additional implementation gap is established by this bounded functional evidence.

## Out of scope

- Public message UI availability or public endpoint reachability: not established by internal registered RPC tests.
- Authentication or actor-spoofing enforcement: not established by this access matrix.
- Message product redesign: outside this reply/forward contract.
