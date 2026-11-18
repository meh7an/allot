#!/bin/bash
# Shared by nested.sh and smoke.sh: a throwaway HOME (~/allot-s03/home) with the spike extension,
# a test token and the mock settings server. Nothing outside that folder is touched.
set -u
SPIKE=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
UUID=allot-settings-spike@mehran.tech
PORT=47960
export HOME=$HOME/allot-s03/home
export XDG_CONFIG_HOME=$HOME/.config
EXT=$HOME/.local/share/gnome-shell/extensions

# A fresh throwaway HOME with the extension, its shared client, a token and an empty log.
prepare_home() {
    rm -rf "$HOME/.allot" "$EXT/$UUID" "$HOME/settings-spike.log" "$HOME/server.log"
    mkdir -p "$HOME/.allot" "$EXT"
    cp -rT "$SPIKE/gnome/$UUID" "$EXT/$UUID"
    cp "$SPIKE/linux/settings-client.mjs" "$EXT/$UUID/"
    echo spike-token > "$HOME/.allot/token"
    echo normal > "$HOME/settings-spike-mode"
}

# Starts the mock server in the background; SERVER_PID is its process.
start_server() {
    python3 "$SPIKE/linux/mock-settingsd.py" "$PORT" > "$HOME/server.log" 2>&1 &
    SERVER_PID=$!
    export SERVER_PID
}

# $1: normal, slow, down or unauthorized.
mode() {
    echo "$1" > "$HOME/settings-spike-mode"
}
