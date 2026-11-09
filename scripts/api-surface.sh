#!/usr/bin/env bash
# Snapshot the public surface as sorted "file: signature" lines.
#
# This working copy has no .git directory (it was copied out of the human's
# tree), so the file list comes from `find` instead of `git ls-files`. The
# pattern and the sort are unchanged from the queue's version.
set -u
cd "$(dirname "$0")/.." || exit 2
out=${1:-docs/api-surface.txt}
{
    for f in $(find src casket-macros/src -name '*.rs' | sort); do
        awk -v F="$f" '
            /^[[:space:]]*pub (fn|struct|enum|trait|type|const|static|mod|use) / {
                sub(/^[[:space:]]+/, ""); print F ": " $0
            }' "$f"
    done
} | sort > "$out"
wc -l < "$out"
