#!/usr/bin/env bash
# Publish the JavaScript packages of this repo to npm, in dependency order:
#   forcefield-sim  →  cytoscape-forcefield
# (forcefield-wasm is published separately from crates/forcefield-wasm/pkg-web
# when its version changes; the Rust crate with `cargo publish -p forcefield`.)
#
# Usage:  scripts/publish-js.sh            # dry run: checks, tests, `npm pack`
#         scripts/publish-js.sh --publish  # the real thing (npm prompts for 2FA in the browser)
#
# Run from a real terminal: npm's two-factor step opens a browser page.
set -euo pipefail
cd "$(dirname "$0")/.."

PUBLISH=0
[[ "${1:-}" == "--publish" ]] && PUBLISH=1

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

say "preflight"
[[ -z "$(git status --porcelain)" ]] || { echo "working tree not clean; commit or stash first"; exit 1; }
git fetch -q origin && [[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/master)" ]] || { echo "HEAD is not pushed to origin/master"; exit 1; }
npm whoami >/dev/null 2>&1 || { echo "not logged in to npm (run: npm login)"; exit 1; }
[[ -f crates/forcefield-wasm/pkg-node/forcefield.js ]] || wasm-pack build crates/forcefield-wasm --release --target nodejs --out-dir pkg-node --out-name forcefield

say "tests"
npm ci --silent
npm test --workspaces

for pkg in js/forcefield-sim js/cytoscape-forcefield; do
  name=$(node -p "require('./$pkg/package.json').name")
  version=$(node -p "require('./$pkg/package.json').version")
  say "$name@$version"
  if npm view "$name@$version" version >/dev/null 2>&1; then
    echo "already on npm, skipping"
    continue
  fi
  if (( PUBLISH )); then
    (cd "$pkg" && npm publish --access public)
    # give the registry a moment before the next package resolves this one
    for i in 1 2 3 4 5 6; do npm view "$name@$version" version >/dev/null 2>&1 && break; sleep 5; done
    git tag -a "$name-v$version" -m "$name $version" 2>/dev/null || true
  else
    (cd "$pkg" && npm pack --dry-run 2>&1 | grep -E "npm notice (name|version|total files|package size)")
  fi
done

if (( PUBLISH )); then
  git push --tags -q
  say "published; tags pushed"
else
  say "dry run only; rerun with --publish"
fi
