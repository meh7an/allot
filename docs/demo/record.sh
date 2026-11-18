#!/usr/bin/env bash
# Records the README demo (docs/images/demo-{dark,light}.gif) from real output, on Windows in
# Git Bash. Two sessions (session.mjs, each with the real allot-mcp) lease a real
# `cargo build --release` of a crate whose build script keeps four threads busy, and the
# second one starts at once on the cores left free; the recording shows
# `allot status --watch` meanwhile. allotd runs under a throwaway USERPROFILE on its own port,
# so a live allotd and ~/.allot are untouched.
# Needs vhs, ttyd and ffmpeg on PATH, node, cargo, and the binaries in ALLOT_DEMO_FROM
# (default target/release, so `cargo build --release` first; ~/.allot/bin takes the installed ones).
#   record.sh setup | take [dark|light] | stop
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
DEMO=${ALLOT_DEMO_DIR:-$HOME/allot-demo}
FROM=${ALLOT_DEMO_FROM:-$REPO/target/release}
BIN=$DEMO/.allot/bin
PORT=47921
# Long enough that the first build is still running when the second session asks.
BUILD_SECS=8
BUILD_THREADS=4
# Seconds after the tape's trigger that each session starts; each asks 2.5 s after it starts.
FIRST_DELAY=0
SECOND_DELAY=2

in_demo_home() {
    env USERPROFILE="$(cygpath -w "$DEMO")" "$@"
}

make_crate() {
    (cd "$DEMO" && cargo new -q --vcs none "$1")
    cat > "$DEMO/$1/build.rs" <<EOF
use std::time::{Duration, Instant};

fn main() {
    let until = Instant::now() + Duration::from_secs($BUILD_SECS);
    let workers: Vec<_> = (0..$BUILD_THREADS)
        .map(|_| std::thread::spawn(move || while Instant::now() < until {}))
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
}
EOF
}

setup() {
    if [ -e "$DEMO" ]; then
        echo "$DEMO already exists; remove it first"
        exit 1
    fi
    mkdir -p "$BIN"
    for name in allot allotd allot-mcp; do
        cp "$FROM/$name.exe" "$BIN/"
    done
    # Two cpu_heavy leases (8 cores by default) do not both fit whole, so the second shrinks.
    # Recording loads the machine too, which can leave less than the default minimum of 4
    # free, so the demo's builds can start on 2.
    printf '[daemon]\nport = %s\n\n[classes.cpu_heavy]\nmin_cores = 2\n' "$PORT" \
        > "$DEMO/.allot/config.toml"
    make_crate web-app
    make_crate api-server
    # allotd keeps the handles it starts with, so leave none open to a caller that captures them.
    in_demo_home "$BIN/allot.exe" daemon start < /dev/null > /dev/null 2>&1
    echo "setup: $(nproc) cores, cpu_heavy min_cores 2, allotd on $PORT"
}

# Cargo and rustup find their homes through USERPROFILE too, so they keep the real ones.
session() {
    CARGO_HOME="${CARGO_HOME:-$USERPROFILE/.cargo}" \
        RUSTUP_HOME="${RUSTUP_HOME:-$USERPROFILE/.rustup}" \
        USERPROFILE="$(cygpath -w "$DEMO")" \
        node "$HERE/session.mjs" "$(cygpath -w "$BIN/allot-mcp.exe")" \
        "$(cygpath -w "$DEMO/$1")" cargo build --release >> "$DEMO/sessions.log" 2>&1
}

play() {
    for _ in $(seq 600); do
        [ -e "$1" ] && break
        sleep 0.1
    done
    [ -e "$1" ] || { echo "the tape never started the sessions"; return 1; }
    touch "$DEMO/web-app/build.rs" "$DEMO/api-server/build.rs"
    sleep "$FIRST_DELAY"
    session web-app &
    sleep "$SECOND_DELAY"
    session api-server &
    wait
}

take_one() {
    theme=$1
    name=$2
    stamp=$(date +%s)
    trigger=$DEMO/go-$name-$stamp
    tape=$DEMO/$name-$stamp.tape
    {
        printf 'Output "%s"\n' "$(cygpath -m "$REPO/docs/images/$name.gif")"
        printf 'Set Theme "%s"\n' "$theme"
        cat "$HERE/demo.tape"
    } > "$tape"
    play "$trigger" &
    ALLOT_DEMO_HOME=$(cygpath -w "$DEMO") ALLOT_DEMO_BIN=$(cygpath -w "$BIN") \
        ALLOT_DEMO_GO=$(cygpath -w "$trigger") vhs "$(cygpath -m "$tape")"
    wait
}

case ${1:-} in
setup)
    setup
    ;;
take)
    # Each take is its own real run, so one can be taken again on its own.
    case ${2:-both} in
    dark | both) take_one "GitHub Dark" demo-dark ;;&
    light | both) take_one "AtomOneLight" demo-light ;;
    esac
    ;;
stop)
    in_demo_home "$BIN/allot.exe" daemon stop
    ;;
*)
    echo "usage: record.sh setup | take [dark|light] | stop"
    exit 2
    ;;
esac
