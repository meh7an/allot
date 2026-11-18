#!/bin/bash
# M0.3 manual check: a nested GNOME Shell in a window (WSLg on Windows, or any GNOME 49+ desktop)
# with only the spike extension, under the throwaway HOME ~/allot-s03/home, against the mock
# server. Close the window to stop everything. Switch the server's mode from a second terminal:
#   echo slow > ~/allot-s03/home/settings-spike-mode     (normal, slow, down, unauthorized)
#   curl -X POST http://127.0.0.1:47960/spike/hand-edit  (an edit made by hand)
# The spike's log is ~/allot-s03/home/settings-spike.log.
set -u
source "$(dirname "$0")/kit.sh"
prepare_home
start_server
dbus-run-session -- bash -c '
    gsettings set org.gnome.shell disable-user-extensions false
    gsettings reset org.gnome.shell disabled-extensions
    gsettings set org.gnome.shell enabled-extensions "[\"'"$UUID"'\"]"
    gnome-shell --devkit --no-x11 > "$HOME/gnome-shell-nested.log" 2>&1
'
kill "$SERVER_PID"
grep -nE "JS ERROR|Spike|spike" "$HOME/gnome-shell-nested.log" | head -30
