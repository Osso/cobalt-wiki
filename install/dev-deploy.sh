#!/usr/bin/env bash
# Fast development deploy of Deepwell and/or Framerail to the Cobalt replica.
# Requires services.cobaltWiki.appDirectory on the host. Builds in the flake's
# deploy shell (same glibc/libmagic store paths as the deployed packages),
# rsyncs only what changed and restarts the service.
#
#   install/dev-deploy.sh [deepwell|framerail|all]     (COBALT_HOST, default sakuin)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST="${COBALT_HOST:-sakuin}"
APP=/var/lib/cobalt-wiki/app
WHAT="${1:-all}"
shell() { nix develop "$ROOT#deploy" --command "$@"; }

# Lazy rerendering keys stored pages to the build: commit plus local changes.
build_id() {
	local id
	id="$(git -C "$ROOT" rev-parse --short=12 HEAD)"
	if ! git -C "$ROOT" diff --quiet HEAD -- deepwell; then
		id="$id-$(git -C "$ROOT" diff HEAD -- deepwell | sha256sum | cut -c1-8)"
	fi
	echo "$id"
}

deploy_deepwell() {
	DEEPWELL_BUILD_ID="$(build_id)" shell cargo build --profile deploy \
		--manifest-path "$ROOT/deepwell/Cargo.toml" --target-dir "$ROOT/deepwell/target/nix-deploy"
	ssh "$HOST" "install -d -o cobalt-wiki -g cobalt-wiki -m 0700 $APP $APP/deepwell"
	rsync -z --chown=cobalt-wiki:cobalt-wiki "$ROOT/deepwell/target/nix-deploy/deploy/deepwell" "$HOST:$APP/deepwell/deepwell.new"
	rsync -rz --delete --chown=cobalt-wiki:cobalt-wiki "$ROOT/deepwell/migrations" "$ROOT/locales" "$HOST:$APP/deepwell/"
	ssh "$HOST" "mv $APP/deepwell/deepwell.new $APP/deepwell/deepwell && systemctl restart cobalt-wiki-deepwell"
}

deploy_framerail() {
	(cd "$ROOT/framerail" && shell pnpm run build)
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

case "$WHAT" in
deepwell) deploy_deepwell ;;
framerail) deploy_framerail ;;
all) deploy_deepwell && deploy_framerail ;;
*) echo "usage: $0 [deepwell|framerail|all]" >&2 && exit 2 ;;
esac

# Smoke check: both services active and a page renders through Deepwell.
ssh "$HOST" 'systemctl is-active cobalt-wiki-deepwell cobalt-wiki-framerail >/dev/null &&
	curl -sf -o /dev/null -H "Content-Type: application/json" http://127.0.0.1:2747/jsonrpc \
	-d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"page_view\",\"params\":{\"site_id\":6000000,\"session_token\":null,\"locales\":[\"en\"],\"route\":{\"slug\":\"roster\",\"extra\":\"\"}}}"' &&
	echo "Deployed $WHAT ($(build_id))."
