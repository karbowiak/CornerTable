#!/usr/bin/env bash
set -euo pipefail

stage=$1
output=$2
appdir=$(mktemp -d)
tools=$(mktemp -d)
trap 'rm -rf -- "$appdir" "$tools"' EXIT

linuxdeploy="$tools/linuxdeploy-x86_64.AppImage"
curl --fail --silent --show-error --location --retry 3 \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage \
    -o "$linuxdeploy"
printf '%s  %s\n' c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d "$linuxdeploy" | sha256sum -c -
chmod +x "$linuxdeploy"

install -Dm755 "$stage/corner-table" "$appdir/usr/bin/corner-table"
docs="$appdir/usr/share/doc/corner-table"
mkdir -p "$docs"
cp -r "$stage/README.md" "$stage/CHANGELOG.md" "$stage/Third-Party Notices" "$docs/"

# Include distribution notices for libraries linuxdeploy may bring into the image.
ldd "$stage/corner-table" | awk '$3 ~ /^\// { print $3 }' | while read -r library; do
    owner=$(dpkg-query -S "$(realpath "$library")" 2>/dev/null | sed -n '1s/: .*//p') || continue
    copyright="/usr/share/doc/${owner%:amd64}/copyright"
    if [[ -f "$copyright" ]]; then
        install -Dm644 "$copyright" "$docs/Third-Party Notices/linux/${owner%:amd64}.txt"
    fi
done

install -Dm644 packaging/macos/icon.png "$tools/corner-table.png"
APPIMAGE_EXTRACT_AND_RUN=1 LDAI_OUTPUT="$(realpath -m "$output")" \
    "$linuxdeploy" --appdir "$appdir" --desktop-file packaging/linux/corner-table.desktop \
    --icon-file "$tools/corner-table.png" --output appimage

test -x "$output"
image=$(realpath "$output")
(cd "$tools" && "$image" --appimage-extract > /dev/null)
test -x "$tools/squashfs-root/AppRun"
test -x "$tools/squashfs-root/usr/bin/corner-table"
cmp "$stage/CHANGELOG.md" "$tools/squashfs-root/usr/share/doc/corner-table/CHANGELOG.md"
