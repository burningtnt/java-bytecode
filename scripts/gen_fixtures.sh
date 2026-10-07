#!/usr/bin/env bash
#
# Regenerate the class-file fixtures with Java 25.
#
# Usage: scripts/gen_fixtures.sh
#
# The generated .class files are checked in so that the test suite can run
# without a JDK; re-run this script whenever the Java sources change or when
# exercising a new class-file feature.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/tests/fixtures/src"
OUT="$ROOT/tests/fixtures/classes"
JAVAC="${JAVAC:-javac}"

if ! command -v "$JAVAC" >/dev/null 2>&1; then
    echo "error: javac not found (set JAVAC)" >&2
    exit 1
fi

echo "using $($JAVAC -version 2>&1)"

rm -rf "$OUT"
mkdir -p "$OUT/main" "$OUT/module"

# Unnamed-module sources (everything outside the module tree).
mapfile -t MAIN_SOURCES < <(find "$SRC/main/java" -name '*.java' | sort)
"$JAVAC" --release 25 -g -parameters -d "$OUT/main" "${MAIN_SOURCES[@]}"

# Named-module sources.
mapfile -t MODULE_SOURCES < <(find "$SRC/module" -name '*.java' | sort)
"$JAVAC" --release 25 -g -parameters -d "$OUT/module" "${MODULE_SOURCES[@]}"

count="$(find "$OUT" -name '*.class' | wc -l)"
echo "generated $count class file(s) under $OUT"
