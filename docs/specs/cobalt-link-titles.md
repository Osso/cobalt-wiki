# Automatic link titles

Deepwell must render automatic page-link labels from target page revisions rather than FTML placeholder text. See [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [ ] Use the current title for automatic labels (`[[[page|]]]`), including nested links.
- [ ] Preserve explicit labels and escape resolved titles as text.
- [ ] Resolve the referenced site, not a same-named page on another site.
- [ ] Display the literal page reference when no live target exists, without fabricating a title.
- [ ] Preserve stored source bytes and original revision attribution when refreshing derived rendering.

## How it works

- [Replica status and remaining boundaries](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `deepwell/src/services/render/link_titles.rs`: collects automatic references and fetches target titles.
- `deepwell/src/services/render/service.rs`: supplies resolved titles during rendering.
- `deepwell/vendor/ftml/src/render/`: renders provided titles using existing escaping.

## Tests asserting this spec

- `deepwell/tests/page_link_titles.rs`: native DB/rendering, nested labels, escaping, explicit labels, missing targets and site boundaries.

## Known gaps (current cycle)

- [ ] Native tests, deployment and existing compiled-page refresh remain pending.

## Out of scope

Includes/ListPages, file URL resolution, missing-link styling, and source ACL parity are separate slices. The whole-site access gate remains mandatory.
