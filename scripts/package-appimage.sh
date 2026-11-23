#!/usr/bin/env bash
set -euo pipefail

stage=$1
output=$2
appdir=$(mktemp -d)
tools=$(mktemp -d)
trap 'rm -rf -- "$appdir" "$tools"' EXIT

download_checked() {
    local url=$1 checksum=$2 destination=$3
    curl --fail --silent --show-error --location --retry 3 "$url" -o "$destination"
    printf '%s  %s\n' "$checksum" "$destination" | sha256sum -c -
}

linuxdeploy="$tools/linuxdeploy-x86_64.AppImage"
runtime="$tools/runtime-x86_64"
download_checked \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage \
    c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d "$linuxdeploy"
download_checked \
    https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64 \
    2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d "$runtime"
chmod +x "$linuxdeploy"

install -Dm755 "$stage/corner-table" "$appdir/usr/bin/corner-table"
docs="$appdir/usr/share/doc/corner-table"
mkdir -p "$docs"
cp -r "$stage/README.md" "$stage/CHANGELOG.md" "$stage/Third-Party Notices" "$docs/"
cp packaging/linux/AppImage-runtime-LICENSE.txt "$docs/Third-Party Notices/"

# Include distribution notices for libraries linuxdeploy may bring into the image.
ldd "$stage/corner-table" | awk '$3 ~ /^\// { print $3 }' | while read -r library; do
    owner=$(dpkg-query -S "$(realpath "$library")" 2>/dev/null | sed -n '1s/: .*//p') || continue
    copyright="/usr/share/doc/${owner%:amd64}/copyright"
    if [[ -f "$copyright" ]]; then
        install -Dm644 "$copyright" "$docs/Third-Party Notices/linux/${owner%:amd64}.txt"
    fi
done

APPIMAGE_EXTRACT_AND_RUN=1 LDAI_OUTPUT="$(realpath -m "$output")" LDAI_RUNTIME_FILE="$runtime" \
    "$linuxdeploy" --appdir "$appdir" --desktop-file packaging/linux/corner-table.desktop \
    --icon-file packaging/linux/corner-table.png --output appimage

test -x "$output"
image=$(realpath "$output")
(cd "$tools" && "$image" --appimage-extract > /dev/null)
test -x "$tools/squashfs-root/AppRun"
test -x "$tools/squashfs-root/usr/bin/corner-table"
cmp "$stage/CHANGELOG.md" "$tools/squashfs-root/usr/share/doc/corner-table/CHANGELOG.md"
