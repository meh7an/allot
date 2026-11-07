#!/bin/sh
# Generates the completion scripts and the man page with the given allot and checks that each
# shell, and man, can read them. A checker that is not installed is skipped with a note, so the
# job decides what it installs.
#
#   check-completions.sh <path to allot>
set -eu

allot=$1
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
failed=0

"$allot" completions bash > "$out/allot.bash"
"$allot" completions zsh > "$out/_allot"
"$allot" completions fish > "$out/allot.fish"
"$allot" man > "$out/allot.1"

# check <name> <checker> <command...>: runs the command when the checker is installed.
check() {
    name=$1
    checker=$2
    shift 2
    if ! command -v "$checker" > /dev/null 2>&1; then
        echo "skipped $name: no $checker"
        return
    fi
    if "$@"; then
        echo "ok $name"
    else
        echo "FAILED $name"
        failed=1
    fi
}

check bash bash bash -n "$out/allot.bash"
check zsh zsh zsh -n "$out/_allot"
check fish fish fish --no-execute "$out/allot.fish"

# man-db's man prints groff's warnings on stderr and still exits 0, so any stderr output fails
# the check. macOS's man (mandoc) has neither --warnings nor -l, and a runner may have man
# without groff, so the job that installs man-db and groff asks for it with CHECK_MAN=1.
# shellcheck disable=SC2317 # called through check
man_page() {
    MANWIDTH=80 man --warnings -l "$out/allot.1" > "$out/man.txt" 2> "$out/man.err" \
        && ! [ -s "$out/man.err" ] && grep -q 'doctor' "$out/man.txt"
}
if [ "${CHECK_MAN:-}" = 1 ]; then
    check "man page" man man_page
    if [ -s "$out/man.err" ]; then
        cat "$out/man.err"
    fi
else
    echo "skipped man page: CHECK_MAN is not 1"
fi

exit "$failed"
