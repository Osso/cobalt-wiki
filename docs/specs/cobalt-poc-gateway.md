# Cobalt POC gateway

`install/nixos/poc-gateway.nix` provides a same-origin entrance to the [native runtime](cobalt-native-runtime.md) while full source ACL compatibility remains unfinished.

## What it must do

- [ ] Listen only on `127.0.0.1:3088`; do not impose gateway HTTP Basic authentication.
- [ ] Route file-server paths to WWS and other requests to Framerail without changing the approved hostname `cobalt-company.sakuin.org`.
- [ ] Replace client-supplied site headers with the provisioned site ID and `cobalt-company` slug; remove the user-ID and gateway Authorization headers before proxying.
- [ ] Preserve cookies and the external HTTPS origin so same-origin sessions and SvelteKit CSRF validation continue to work.

## How it works

- [Native runtime integration](../wiki/systems/cobalt-native-runtime.md)

## Implementation inventory

- `install/nixos/poc-gateway.nix` — standalone nginx configuration and namespaced systemd service.
- `install/nixos/module.nix` — imports the optional gateway module.

Enable `services.cobaltWiki.pocGateway.enable` and supply `siteId` from the actual provisioned database. The runtime's `mainDomain` and `filesDomain` must both be `sakuin.org`: Deepwell prefixes the latter with the site slug, producing the approved hostname. The gateway overrides Framerail's `ORIGIN` to that full hostname. No generated Deepwell Caddy routing is used.

The gateway runs inside the existing capped wiki slice. Its standalone nginx instance does not enable the host nginx module or introduce public listeners. Main provisions the `cobalt` login hash separately; the existing Sakuin Cloudflare Tunnel maps public HTTPS traffic for `cobalt-company.sakuin.org` to this loopback gateway.

## Tests asserting this spec

Targeted Nix evaluation covers configuration generation. The 29-route Basic-auth denial and public-HTTPS challenge checks below are historical proof from before commit `3d4a27740`. At 2026-09-25 00:57 UTC, deployed public HTTPS `/` and `/icons` returned anonymous `200` without `WWW-Authenticate`, with no-index intact; this narrow proof does not establish application ACL parity. See [current proof](../wiki/systems/cobalt-replica-status.md).

## Known gaps (current cycle)

- [x] Production bootstrap assigned site ID `6000000` on 2026-09-22. Historical deployment used a protected loopback gateway for that ID.
- [x] Historical proof before `3d4a27740`: the deployed gateway returned `401` for 29 unauthenticated or wrong-password route/method checks, including pages, assets, file/download paths, robots, and `.well-known`.
- [x] Historical proof before `3d4a27740`: WWS file routing served imported images over public HTTPS with archive-matching hashes. The positional `site_domain` and trusted target-server-header fixes were deployed. WWS's authenticated robots implementation remains unsupported; unauthenticated robots requests received the shared challenge and no-index header.
- [ ] Invalid forged session-cookie handling returns a server error; it is not an authentication bypass, but authenticated session behavior is not accepted.
- [x] Historical proof before `3d4a27740`: runtime htpasswd ownership/readability was corrected for `cobalt-wiki`.
- [x] Historical proof before `3d4a27740`: public HTTPS ingress used the existing Sakuin Cloudflare Tunnel as a proxied CNAME to the loopback gateway. Browser-like requests received the gateway's `401 Basic` challenge. Python's default client was blocked upstream with Cloudflare `1010`.
- [x] Commit `3d4a27740` was deployed by 2026-09-25 00:57 UTC. Anonymous public HTTPS `/` and `/icons` returned `200` without `WWW-Authenticate`; no-index remained intact. This does not establish application ACL parity.
- [x] Full source/attachment import completed and file routing was exercised. Historical route proof used gateway authentication; public ingress does not establish source ACL parity.
- [ ] Source ACLs remain unfinished. WWS per-page authorization (Page/View per viewer for files and text blocks) is committed, not deployed; see [media routing](cobalt-media-routing.md). Removing gateway Basic Auth does not establish source-permission parity.

## Out of scope

Secrets, bootstrap/import, host integration, and DNS belong to main. Commit `3d4a27740` removes gateway-wide HTTP Basic Auth; it does not change application login, sessions, permissions, data, no-index, or HTTPS requirements, and deployment does not establish ACL parity.
