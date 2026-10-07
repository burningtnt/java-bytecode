#!/usr/bin/env bash
#
# Fetch an OpenJDK 25 corpus of .class files for the bytecode tests.
#
# Downloads the latest Temurin 25 GA build from Adoptium, verifies its SHA-256,
# extracts the runtime image with `jimage`, and leaves the classes in
#
#   tests/fixtures/jdk-corpus/            (default)
#
# or $JAVA_BYTECODE_CORPUS_DIR when set. The directory is a generated artifact
# and is not committed.
#
# Usage:
#   scripts/fetch_jdk_corpus.sh [--force]
#
# The tests look for this directory automatically; when it is absent they print
# a warning and skip (see tests/bytecode_corpus.rs).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${JAVA_BYTECODE_CORPUS_DIR:-$ROOT/tests/fixtures/jdk-corpus}"

FORCE=0
if [[ "${1:-}" == "--force" ]]; then
    FORCE=1
fi

have_classes() {
    [[ -d "$DEST" ]] && [[ -n "$(find "$DEST" -name '*.class' -print -quit 2>/dev/null)" ]]
}

if have_classes && [[ $FORCE -eq 0 ]]; then
    echo "corpus already present: $DEST (use --force to refresh)"
    exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

api="https://api.adoptium.net/v3/assets/latest/25/hotspot?architecture=x64&image_type=jdk&os=linux&vendor=eclipse"
echo "resolving latest Temurin 25 from Adoptium..."
curl -fsSL "$api" -o "$work/assets.json"

# shellcheck disable=SC2046
set -- $(python3 - "$work/assets.json" <<'PY'
import json, sys
binary = json.load(open(sys.argv[1]))[0]["binary"]
package = binary["package"]
print(package["link"], package["checksum"], package["name"])
PY
)
url="$1"
checksum="$2"
name="$3"

echo "downloading $name"
curl -fsSL -o "$work/jdk.tar.gz" "$url"
echo "$checksum  $work/jdk.tar.gz" | sha256sum -c -

mkdir -p "$work/jdk"
tar -xzf "$work/jdk.tar.gz" -C "$work/jdk" --strip-components=1

rm -rf "$DEST"
mkdir -p "$DEST"
"$work/jdk/bin/jimage" extract --dir "$DEST" "$work/jdk/lib/modules" 2>/dev/null

count="$(find "$DEST" -name '*.class' | wc -l)"
echo "extracted $count class file(s) to $DEST"
