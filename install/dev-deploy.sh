#!/usr/bin/env bash
# Application-only deploy of Deepwell, Framerail and WWS to the Cobalt replica.
# Requires services.cobaltWiki.appDirectory on the host. Builds in the flake's
# deploy shell (same glibc/libmagic store paths as the deployed packages),
# rsyncs only what changed and restarts the service.
#
#   install/dev-deploy.sh [deepwell|framerail|wws|all] (COBALT_HOST, default sakuin)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST="${COBALT_HOST:-sakuin}"
APP=/var/lib/cobalt-wiki/app
WHAT="${1:-all}"
# Personal cargo config may carry linker flags for other linkers; build clean.
shell() { CARGO_ENCODED_RUSTFLAGS="" nix develop "$ROOT#deploy" --command "$@"; }

# Deployment label only; Deepwell's build script fingerprints rendering inputs.
release_id() {
	local id
	id="$(git -C "$ROOT" rev-parse --short=12 HEAD)"
	if ! git -C "$ROOT" diff --quiet HEAD -- deepwell; then
		id="$id-$(git -C "$ROOT" diff HEAD -- deepwell | sha256sum | cut -c1-8)"
	fi
	echo "$id"
}

deploy_deepwell() {
	shell cargo build --profile deploy \
		--manifest-path "$ROOT/deepwell/Cargo.toml" --target-dir "$ROOT/deepwell/target/nix-deploy"
	ssh "$HOST" "install -d -o cobalt-wiki -g cobalt-wiki -m 0700 $APP $APP/deepwell"
	rsync -z --chown=cobalt-wiki:cobalt-wiki "$ROOT/deepwell/target/nix-deploy/deploy/deepwell" "$HOST:$APP/deepwell/deepwell.new"
	rsync -rz --delete --chown=cobalt-wiki:cobalt-wiki "$ROOT/deepwell/migrations" "$ROOT/locales" "$HOST:$APP/deepwell/"
	ssh "$HOST" "mv $APP/deepwell/deepwell.new $APP/deepwell/deepwell && systemctl restart cobalt-wiki-deepwell"
}

# pnpm run through a symlinked node_modules can write package links whose
# literal targets climb out of the tree; they resolve locally but not on the
# host. Compare targets lexically, since resolving symlinks hides the escape.
check_node_modules_links() {
	local modules escaped
	modules="$(realpath "$ROOT/framerail/node_modules")"
	escaped="$(find "$modules/" -type l -printf '%h/%l\n' | xargs -r -d '\n' realpath -ms |
		grep -v "^$modules/" || true)"
	if [[ -n "$escaped" ]]; then
		echo "node_modules links point outside $modules; reinstall from the real path:" >&2
		head -5 <<<"$escaped" >&2
		exit 1
	fi
}

deploy_framerail() {
	check_node_modules_links
	(cd "$ROOT/framerail" && shell pnpm --config.verify-deps-before-run=false run build)
	ssh "$HOST" "install -d -o cobalt-wiki -g cobalt-wiki -m 0700 $APP $APP/framerail"
	# Dependencies change rarely; resend them only when the lockfile does.
	local lock
	lock="$(sha256sum "$ROOT/framerail/pnpm-lock.yaml" | cut -c1-64)"
	if [[ "$(ssh "$HOST" "cat $APP/framerail/.lock-sha256 2>/dev/null || true")" != "$lock" ]]; then
		# node_modules may itself be a symlink; pnpm's links inside it are relative.
		rsync -rlz --delete --chown=cobalt-wiki:cobalt-wiki "$ROOT/framerail/node_modules/" "$HOST:$APP/framerail/node_modules/"
		rsync -z --chown=cobalt-wiki:cobalt-wiki "$ROOT/framerail/package.json" "$HOST:$APP/framerail/"
		ssh "$HOST" "echo $lock > $APP/framerail/.lock-sha256"
	fi
	rsync -rz --delete --chown=cobalt-wiki:cobalt-wiki "$ROOT/framerail/build/" "$HOST:$APP/framerail/build/"
	ssh "$HOST" "systemctl restart cobalt-wiki-framerail"
}

build_and_copy_wws() {
	WWS_PACKAGE="$(nix build "$ROOT#wws" --no-link --print-out-paths)"
	nix copy --to "ssh://$HOST" "$WWS_PACKAGE"
}

activate_wws() {
	local package="$1"
	local dropin=/etc/systemd/system/cobalt-wiki-wws.service.d/release.conf
	# Pin only the executable; the NixOS unit retains all other service policy.
	printf '[Service]\nExecStart=\nExecStart=%s/bin/wws\n' "$package" |
		ssh "$HOST" "install -d -m 0755 ${dropin%/*} && tee $dropin >/dev/null && systemctl daemon-reload && systemctl restart cobalt-wiki-wws"
}

case "$WHAT" in
deepwell) deploy_deepwell ;;
framerail) deploy_framerail ;;
wws)
	build_and_copy_wws
	activate_wws "$WWS_PACKAGE"
	;;
all)
	build_and_copy_wws
	deploy_deepwell
	deploy_framerail
	activate_wws "$WWS_PACKAGE"
	;;
*) echo "usage: $0 [deepwell|framerail|wws|all]" >&2 && exit 2 ;;
esac

# Reconnect the dependent gateway, then check all application services.
ssh "$HOST" 'systemctl restart cobalt-wiki-gateway &&
	systemctl is-active cobalt-wiki-deepwell cobalt-wiki-framerail cobalt-wiki-wws cobalt-wiki-gateway >/dev/null &&
	curl -sf -o /dev/null -H "Content-Type: application/json" http://127.0.0.1:2747/jsonrpc \
	-d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"page_view\",\"params\":{\"site_id\":6000000,\"session_token\":null,\"locales\":[\"en\"],\"route\":{\"slug\":\"roster\",\"extra\":\"\"}}}"' &&
	echo "Deployed $WHAT ($(release_id))."
