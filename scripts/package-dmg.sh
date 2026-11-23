#!/usr/bin/env bash
set -euo pipefail

stage=$1
output=$2
image_stage=$(mktemp -d)
mount=$(mktemp -d)
trap 'hdiutil detach "$mount" >/dev/null 2>&1 || true; rm -rf -- "$mount" "$image_stage"' EXIT

ditto "$stage" "$image_stage"
ln -s /Applications "$image_stage/Applications"
hdiutil create -volname 'Corner Table' -srcfolder "$image_stage" -format UDZO -fs HFS+ "$output"
hdiutil verify "$output"
hdiutil attach -readonly -nobrowse -noautoopen -mountpoint "$mount" "$output"
codesign --verify --strict "$mount/Corner Table.app"
test "$(readlink "$mount/Applications")" = /Applications
test -s "$mount/Third-Party Notices/README.md"
hdiutil detach "$mount"
