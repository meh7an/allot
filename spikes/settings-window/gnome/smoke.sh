#!/bin/bash
# M0.3 smoke run: the spike extension in a headless GNOME Shell under the throwaway HOME
# ~/allot-s03/home, its prefs window driven through AT-SPI, against the mock server.
# Usage: bash smoke.sh <screenshot dir>
set -u
DIR=$(cd "$(dirname "$0")" && pwd)
source "$DIR/kit.sh"
export SHOTS=$1 DIR SPIKE UUID PORT PROBE=${PROBE:-}
unset WAYLAND_DISPLAY DISPLAY
prepare_home
mkdir -p "$SHOTS"
# Unsafe mode opens Eval and Screenshot to the session script; this helper exists only here.
HELPER=$EXT/allot-s03-helper@mehran.tech
mkdir -p "$HELPER"
cat > "$HELPER/metadata.json" << 'EOF'
{"name": "Allot S03 helper", "description": "Unsafe mode for the M0.3 smoke run",
 "uuid": "allot-s03-helper@mehran.tech", "shell-version": ["48", "49", "50", "51"]}
EOF
cat > "$HELPER/extension.js" << 'EOF'
import { Extension } from "resource:///org/gnome/shell/extensions/extension.js";

export default class Helper extends Extension {
  enable() {
    global.context.unsafe_mode = true;
  }

  disable() {
    global.context.unsafe_mode = false;
  }
}
EOF
start_server
dbus-run-session -- bash "$DIR/smoke-session.sh"
kill "$SERVER_PID" 2> /dev/null
echo "== spike log"
cat "$HOME/settings-spike.log"
echo "== server requests"
cat "$HOME/server.log"
echo "== shell errors"
grep -nE "JS ERROR|JS WARNING|Already showing|spike" "$HOME/gnome-shell.log" | head -30
pgrep -l -f "gnome-shell|mock-settingsd" || echo "nothing left running"
