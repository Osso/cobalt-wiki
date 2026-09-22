# Same-site media routing

FTML must render same-site attachment references through the deployed site's protected file route, not an unconfigured external hostname. See [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [ ] Emit same-origin file URLs for current-page, same-site other-page, and explicitly same-site references.
- [ ] Preserve explicit external URLs and unrelated cross-site references.
- [ ] Preserve source bytes and revision identity while refreshing derived HTML.
- [ ] Serve actual imported image bytes behind authentication and no-index controls.

## How it works

- [Protected file gateway](cobalt-poc-gateway.md)

## Implementation inventory

- `deepwell/vendor/ftml/src/render/handle.rs`: selects same-origin routes for same-site media.

## Tests asserting this spec

- `deepwell/tests/page_media_urls.rs`: native rendering of local, explicit same-site, external and cross-site image references.
- FTML AST fixtures: image/audio/video output contracts.

## Known gaps (current cycle)

- [ ] Native tests, deployment, affected rendering refresh and browser image checks.
- [ ] Two homepage images still depend on unresolved template variables; routing must not pretend their requested assets exist.

## Out of scope

External asset acquisition, cross-site routing redesign, template expansion, and source ACL parity remain separate work. Shared POC authentication stays mandatory.
