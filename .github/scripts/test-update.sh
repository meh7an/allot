#!/usr/bin/env bash
# Tests `allot update` end to end on Linux and macOS: an older real release is installed into a
# throwaway HOME and runs allotd with a lease, this tree's build is served as a release on a
# loopback port, and the allot built from this tree updates the install to it. A wrong checksum
# comes first and must change nothing. Run from the workspace root after
# `cargo build --release --locked`; OLD_VERSION names the release to start from.
#   test-update.sh
set -euo pipefail

old=${OLD_VERSION:-0.6.0}
root=$(pwd)
tree=$root/target/release
work=$(mktemp -d)
home=$work/home
server_port=47933
daemon_port=47934
api=http://127.0.0.1:$daemon_port
failures=0

cleanup() {
    # The throwaway install's allotd, then what this script started.
    in_home "$home/.allot/bin/allot" uninstall --purge >/dev/null 2>&1 || true
    kill "${holder:-}" "${server:-}" 2>/dev/null || true
    if ((failures)) && [[ -f $work/server.log ]]; then
        echo '--- the release server log' >&2
        cat "$work/server.log" >&2
    fi
    rm -rf "$work"
}
trap cleanup EXIT

check() {
    printf 'ok: %s\n' "$1"
}

problem() {
    printf 'FAIL: %s\n' "$1" >&2
    failures=$((failures + 1))
}

die() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

sha256() {
    if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

# Runs a command in the throwaway home. ALLOT_CLAUDE points nowhere, so nothing is registered
# with a real Claude Code.
in_home() {
    env HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
        ALLOT_CLAUDE=/nonexistent ALLOT_UPDATE_BASE="http://127.0.0.1:$server_port/releases" \
        "$@"
}

health() {
    curl -fsS "$api/health"
}

for exe in allot allotd allot-mcp; do
    [[ -x $tree/$exe ]] || die "$tree/$exe is missing: run cargo build --release --locked first"
done
version=$("$tree/allot" --version | awk '{ print $2 }')
target=$(rustc -vV | sed -n 's/^host: //p')
name=allot-$version-$target
[[ $version != "$old" ]] || die "this tree is $version, the release to start from"

echo "--- this tree as release $version for $target"
download=$work/release/download/v$version
mkdir -p "$download" "$work/stage/$name"
cp "$tree/allot" "$tree/allotd" "$tree/allot-mcp" "$work/stage/$name/"
# COPYFILE_DISABLE: macOS tar would add a ._ file for every extended attribute.
COPYFILE_DISABLE=1 tar -czf "$download/$name.tar.gz" -C "$work/stage" "$name"
(cd "$download" && sha256 "$name.tar.gz" >"$name.tar.gz.sha256")
good_sum=$(cat "$download/$name.tar.gz.sha256")
python3 "$root/.github/scripts/serve-release.py" "$work/release" "$server_port" "v$version" \
    >"$work/server.log" 2>&1 &
server=$!
for _ in $(seq 50); do
    [[ $(curl -s -o /dev/null -w '%{http_code}' \
        "http://127.0.0.1:$server_port/releases/latest") == 302 ]] && break
    sleep 0.1
done

echo "--- release $old, installed with install.sh into a throwaway HOME"
mkdir -p "$home/.local/bin" "$home/.allot"
# A port of its own, and a heartbeat timeout longer than this script, since nothing here sends
# heartbeats.
printf '[daemon]\nport = %s\n\n[leases]\nheartbeat_ttl_secs = 900\n' "$daemon_port" \
    >"$home/.allot/config.toml"
in_home sh "$root/install.sh" --version "$old" --no-claude --no-autostart
installed=$("$home/.allot/bin/allot" --version)
[[ $installed == "allot $old" ]] || die "expected allot $old, got $installed"
in_home "$home/.allot/bin/allot" daemon start
old_pid=$(health | jq -r .pid)
[[ $(health | jq -r .version) == "$old" ]] || die "allotd is not $old"
check "allotd $old runs, pid $old_pid"

echo '--- a lease, held by a process that outlives the update'
# The holder is the shell that ran the probe and then becomes `sleep`: the probe's second
# ancestry line is its pid and start time, the way the daemon identifies a session's process.
report=$work/holder.txt
sh -c '"$1" probe --out "$2"; exec sleep 3600' sh "$tree/allot" "$report" &
holder=$!
for _ in $(seq 100); do
    [[ -s $report ]] && break
    sleep 0.1
done
read -r holder_pid holder_start < <(
    awk '/^-- ancestry/ { getline; getline; print $1, $3; exit }' "$report"
) || die "the probe report has no ancestry: $(cat "$report")"
[[ $holder_pid == "$holder" ]] ||
    die "the probe found holder $holder_pid, not $holder: $(cat "$report")"
body=$(printf '{"holder":{"session":"e2e-update","pid":%s,"pid_start_time":%s},"profile":{"class":"light"},"kind":"normal","label":"e2e update","wait_timeout_secs":10}' \
    "$holder_pid" "$holder_start")
response=$(curl -fsS -X POST -H "Authorization: Bearer $(cat "$home/.allot/token")" \
    -H 'Content-Type: application/json' -d "$body" "$api/leases")
lease=$(jq -r .lease.id <<<"$response")
[[ $(jq -r .lease.state <<<"$response") == active ]] || die "the lease is not active: $response"
check "lease $lease is active"

echo '--- a wrong checksum changes nothing'
printf '%064d  %s\n' 0 "$name.tar.gz" >"$download/$name.tar.gz.sha256"
if output=$(in_home "$tree/allot" update 2>&1); then
    problem "an update with a wrong checksum succeeded: $output"
elif [[ $output != *'does not match its checksum'* ]]; then
    problem "a wrong checksum failed for another reason: $output"
else
    check 'a wrong checksum is refused'
fi
if [[ $("$home/.allot/bin/allot" --version) == "allot $old" ]] &&
    [[ $(health | jq -r .pid) == "$old_pid" ]]; then
    check "allot and allotd are still $old"
else
    problem 'the failed update changed the install'
fi
[[ $(jq -r .state "$home/.allot/update-install.json") == failed ]] ||
    problem 'the failed update left no failed record'
printf '%s\n' "$good_sum" >"$download/$name.tar.gz.sha256"

echo "--- allot update to $version"
output=$(in_home "$tree/allot" update 2>&1) || {
    printf '%s\n' "$output" >&2
    die 'allot update failed'
}
printf '%s\n' "$output"
installed=$("$home/.allot/bin/allot" --version)
if [[ $installed == "allot $version" ]]; then
    check "the install is $installed"
else
    problem "expected allot $version, got $installed"
fi
new_health=$(health)
new_pid=$(jq -r .pid <<<"$new_health")
if [[ $(jq -r .version <<<"$new_health") == "$version" && $new_pid != "$old_pid" ]]; then
    check "allotd restarted: $version, pid $new_pid after pid $old_pid"
else
    problem "allotd did not restart on $version: $new_health"
fi
leases=$(in_home "$home/.allot/bin/allot" leases --json)
if jq -e --argjson id "$lease" '.[] | select(.id == $id and .state == "active")' \
    <<<"$leases" >/dev/null; then
    check "lease $lease is still active after the restart"
else
    problem "lease $lease was not restored: $leases"
fi
[[ $(jq -r .state "$home/.allot/update-install.json") == succeeded ]] ||
    problem 'the update left no succeeded record'
[[ $output == *'restarted allotd on the new build'* ]] ||
    problem 'the update did not say it restarted allotd'
if [[ -e $home/.allot/updates/$version ]]; then
    problem "the download folder was left behind"
fi

if ((failures)); then
    echo "$failures check(s) failed" >&2
    exit 1
fi
echo 'all checks passed'
