#!/usr/bin/env bash
# Check that every vendor directory has the same three-part shape:
#   mod.rs                       (the module door)
#   at least one of client.rs, scan.rs, session.rs  (the entry point)
#
# Prints one FAIL line per miss and exits 1 if any vendor is off-shape.
# Prints "module shape: ok" and exits 0 when nothing is missing.
#
# This is a report, not a fixer: a vendor whose entry file is named
# driver.rs or webvisit.rs is real debt (see the ITERATION-LOG entry for
# Task 06), and renaming protocol files is the human's call.
set -u
cd "$(dirname "$0")/.." || exit 2

root=src/vendors
if [ ! -d "$root" ]; then
    echo "FAIL $root: directory not found (run from the crate root)" >&2
    exit 2
fi

fail=0
for dir in "$root"/*/; do
    name=$(basename "$dir")

    if [ ! -f "$dir/mod.rs" ]; then
        echo "FAIL $name: missing mod.rs"
        fail=1
    fi

    if [ ! -f "$dir/client.rs" ] && [ ! -f "$dir/scan.rs" ] && [ ! -f "$dir/session.rs" ]; then
        found=$(find "$dir" -maxdepth 1 -name '*.rs' -printf '%f ' 2>/dev/null | sed 's/ *$//')
        echo "FAIL $name: no client.rs, scan.rs or session.rs (has: ${found:-nothing})"
        fail=1
    fi
done

if [ "$fail" -eq 0 ]; then
    echo "module shape: ok"
    exit 0
fi

exit 1
