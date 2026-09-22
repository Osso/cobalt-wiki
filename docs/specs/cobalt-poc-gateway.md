# Cobalt POC gateway

`install/nixos/poc-gateway.nix` provides a restricted, same-origin entrance to the [native runtime](cobalt-native-runtime.md) while full source ACL compatibility remains unfinished.

## What it must do

- [ ] Listen only on `127.0.0.1:3088` and require HTTP Basic authentication on every request, including files, downloads, legacy redirects, assets, errors, and health paths.
- [ ] Read password hashes only from a runtime htpasswd file outside the Nix store; never embed credentials in generated configuration.
- [ ] Route file-server paths to WWS and other requests to Framerail without changing the approved hostname `cobalt-company.sakuin.org`.
- [ ] Replace client-supplied site headers with the provisioned site ID and `cobalt-company` slug; remove the user-ID and gateway Authorization headers before proxying.
- [ ] Preserve cookies and the external HTTPS origin so same-origin sessions and SvelteKit CSRF validation continue to work.

## How it works

- [Native runtime integration](../wiki/systems/cobalt-native-runtime.md)

## Implementation inventory

- `install/nixos/poc-gateway.nix` — standalone nginx configuration and namespaced systemd service.
- `install/nixos/module.nix` — imports the optional gateway module.

Enable `services.cobaltWiki.pocGateway.enable`, supply `siteId` from the actual provisioned database, and set `htpasswdFile` to a file readable by the `cobalt-wiki` account. The runtime's `mainDomain` and `filesDomain` must both be `sakuin.org`: Deepwell prefixes the latter with the site slug, producing the approved hostname. The gateway overrides Framerail's `ORIGIN` to that full hostname. No generated Deepwell Caddy routing is used.

The gateway runs inside the existing capped wiki slice. Its standalone nginx instance does not enable the host nginx module or introduce public listeners. Main provisions the `cobalt` login hash and Cloudflare Tunnel ingress separately; external traffic must reach this gateway only through HTTPS.

## Tests asserting this spec

Targeted Nix evaluation covers configuration generation. Main additionally verified deployed loopback Basic-auth denial across 29 unauthenticated or wrong-password route/method checks, including an immutable asset; authenticated WWS behavior remains unproven.

## Known gaps (current cycle)

- [x] Production bootstrap assigned site ID `6000000` on 2026-09-22. The protected loopback gateway is deployed for that ID.
- [x] Deployed gateway returned `401` for 29 unauthenticated or wrong-password route/method checks, including pages, assets, file/download paths, robots, and `.well-known`.
- [ ] Authenticated WWS routes currently fail because WWS sends the valid positional-array `site_domain` RPC request but Deepwell rejects it. The compatibility fix awaits main deployment of `ff7dcf3` and repeat acceptance.
- [ ] Invalid forged session-cookie handling returns a server error; it is not an authentication bypass, but authenticated session behavior is not accepted.
- [x] Runtime htpasswd ownership/readability was corrected for `cobalt-wiki`.
- [ ] Add public tunnel ingress and DNS only after deployed authenticated routing acceptance. No public ingress, DNS, or source import is complete.
- [ ] Source ACLs and WWS per-page authorization remain unfinished. The POC gate grants its holders access to the whole POC; it is not source-permission parity.

## Out of scope

Secrets, bootstrap/import, host integration, DNS, and deployment belong to main. This temporary whole-site gate is required because the full dataset contains private content; retire it only after source page/file ACL parity is proven and public exposure is explicitly approved.
