# Cobalt POC gateway

`install/nixos/poc-gateway.nix` provides a restricted, same-origin entrance to the [native runtime](cobalt-native-runtime.md) while full source ACL compatibility remains unfinished.

## What it must do

- [ ] Listen only on `127.0.0.1:3088` and require HTTP Basic authentication on every request, including files, downloads, legacy redirects, assets, errors, and health paths.
- [ ] Read password hashes only from a runtime htpasswd file outside the Nix store; never embed credentials in generated configuration.
- [ ] Route file-server paths to WWS and other requests to Framerail without changing the approved hostname `cobalt-company.sakuin.org`.
- [ ] Replace client-supplied site headers with the provisioned site ID and `cobalt-company` slug; remove the user-ID and gateway Authorization headers before proxying.
- [ ] Preserve cookies and the external HTTPS origin so same-origin sessions and SvelteKit CSRF validation continue to work.

## Approved local-only exception

The unmanaged full-preview gateway at `127.0.0.1:3090` has no nginx Basic Auth as of 2026-09-24. This approved localhost-only development exception removed only `auth_basic` and `auth_basic_user_file` from `/home/osso/.local/share/cobalt-wiki/local-full/nginx/nginx.conf`; it retains loopback binding, trusted headers, application authentication, and every other setting. Independent metadata proof confirms only `127.0.0.1:3090` listened and its unauthenticated homepage/login UI returned `200`; this does not prove application permissions. Main confirmed tracked production gateway configuration remains unchanged. It does not modify this production gateway contract, including the `127.0.0.1:3088` Basic Auth requirement.

## How it works

- [Native runtime integration](../wiki/systems/cobalt-native-runtime.md)

## Implementation inventory

- `install/nixos/poc-gateway.nix` — standalone nginx configuration and namespaced systemd service.
- `install/nixos/module.nix` — imports the optional gateway module.

Enable `services.cobaltWiki.pocGateway.enable`, supply `siteId` from the actual provisioned database, and set `htpasswdFile` to a file readable by the `cobalt-wiki` account. The runtime's `mainDomain` and `filesDomain` must both be `sakuin.org`: Deepwell prefixes the latter with the site slug, producing the approved hostname. The gateway overrides Framerail's `ORIGIN` to that full hostname. No generated Deepwell Caddy routing is used.

The gateway runs inside the existing capped wiki slice. Its standalone nginx instance does not enable the host nginx module or introduce public listeners. Main provisions the `cobalt` login hash separately; the existing Sakuin Cloudflare Tunnel maps public HTTPS traffic for `cobalt-company.sakuin.org` to this loopback gateway.

## Tests asserting this spec

Targeted Nix evaluation covers configuration generation. Main additionally verified deployed loopback Basic-auth denial across 29 unauthenticated or wrong-password route/method checks, including an immutable asset. Public HTTPS tests verify authentication/no-index headers and archive-matching theme-image downloads; see [current proof](../wiki/systems/cobalt-replica-status.md).

## Known gaps (current cycle)

- [x] Production bootstrap assigned site ID `6000000` on 2026-09-22. The protected loopback gateway is deployed for that ID.
- [x] Deployed gateway returned `401` for 29 unauthenticated or wrong-password route/method checks, including pages, assets, file/download paths, robots, and `.well-known`.
- [x] Authenticated WWS file routing serves imported images over public HTTPS with archive-matching hashes. The positional `site_domain` and trusted target-server-header fixes are deployed. WWS's authenticated robots implementation remains unsupported; unauthenticated robots requests receive the shared challenge and no-index header.
- [ ] Invalid forged session-cookie handling returns a server error; it is not an authentication bypass, but authenticated session behavior is not accepted.
- [x] Runtime htpasswd ownership/readability was corrected for `cobalt-wiki`.
- [x] Public HTTPS ingress is configured through the existing Sakuin Cloudflare Tunnel as a proxied CNAME to the loopback gateway. Browser-like requests receive the gateway's `401 Basic` challenge. Python's default client is blocked upstream with Cloudflare `1010`.
- [x] Full source/attachment import completed and authenticated file routing was exercised. Public ingress does not establish source ACL parity.
- [ ] Source ACLs remain unfinished. WWS per-page authorization (Page/View per viewer for files and text blocks) is committed, not deployed; see [media routing](cobalt-media-routing.md). The POC gate grants its holders access to the whole POC; it is not source-permission parity.

## Out of scope

Secrets, bootstrap/import, host integration, DNS, and deployment belong to main. This temporary whole-site gate is required because the full dataset contains private content; retire it only after source page/file ACL parity is proven and public exposure is explicitly approved.
