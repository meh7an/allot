#!/usr/bin/env bash
# M0.3 smoke run on a Plasma 6 machine: the spike widget in a fresh throwaway nested session
# (nested.sh), both Settings hosts driven through AT-SPI against the mock server. Screenshots go
# to ~/allot-s03/shots; the widget's log lines and the server's requests are printed at the end.
set -u
N="bash $(cd "$(dirname "$0")" && pwd)/nested.sh"
W="Allot Settings Spike"
ROOT="$HOME/allot-s03"

step() {
  echo
  echo "== $(date +%T.%3N) $1"
}
d() {
  $N drive "$@"
}
# The config dialog's window title, which Plasma sets.
dialog_title() {
  d windows | grep -F "Allot" | grep -vF "'$W'" | sed -E "s/^[^']*'(.*)'$/\1/" | head -1
}
pick_category() {
  d press "$D" "$1"
  sleep 1.5
}

$N stop > /dev/null
$N unserve
rm -rf "$ROOT/home" "$ROOT/shots"
$N widget
$N start > /dev/null
$N serve
sleep 4
$N panel
$N a11y
sleep 3

step "own window: open"
$N command open-window
sleep 4
$N shot window-open
step "own window: edit headroom to 20"
d set "$W" Headroom 20
sleep 2
step "own window: 100 is refused and goes back"
d set "$W" Headroom 100
sleep 2
d value "$W" Headroom
$N shot window-invalid
step "own window: a hand edit, then an edit: conflict, then Apply Mine"
$N hand-edit
d set "$W" "Estimate margin" 12
sleep 2
$N shot window-conflict
d press "$W" "Apply Mine"
sleep 2
d value "$W" "Estimate margin"
step "own window: the port needs a restart"
d set "$W" Port 47999
sleep 2
d dump "$W" | grep -iE "restart"
step "own window: a widget setting, saved at once"
d press "$W" "Show CPU (saved at once)"
sleep 2
grep -rh "showCpu" "$ROOT/home/.config/plasma-org.kde.plasma.desktop-appletsrc" || echo "showCpu not in the config file yet"
step "own window: an edit just before closing"
d set "$W" Headroom 33
$N command close-window
sleep 3
d windows | grep -F "'$W'" || echo "window hidden"
step "own window: reopen shows the same window"
$N command open-window
sleep 3
d value "$W" Headroom
step "own window: allotd down, then back"
$N mode down
d press "$W" Retry
sleep 2
d dump "$W" | grep -iE "answering|connected"
$N shot window-down
$N mode normal
d press "$W" Retry
sleep 2
$N command close-window
sleep 1

step "config dialog: open"
$N command open-dialog
sleep 5
D=$(dialog_title)
echo "dialog title: '$D'"
d dump "$D" > "$ROOT/shots/dump-dialog.txt"
$N shot dialog-open
step "config dialog: an edit turns Apply on, Apply writes"
d dump "$D" | grep -E "'(Apply|OK|Cancel)'"
d set "$D" Headroom 25
sleep 1
d dump "$D" | grep -E "'(Apply|OK|Cancel)'"
d press "$D" Apply
sleep 2
step "config dialog: switching categories rebuilds the page"
pick_category Panel
pick_category Admission
d value "$D" Headroom
step "config dialog: switching with unsaved edits"
d set "$D" Headroom 27
sleep 1
pick_category Panel
d windows | grep -F "Allot"
$N shot dialog-switch-unsaved
d dump "$D" | grep -E "'(Apply|Discard|Cancel)'" | head
step "config dialog: hand writes to the widget, then OK"
d press "$D" Apply
sleep 2
pick_category Panel
d press "$D" "Send allotd's changes through the widget"
d press "$D" Apply
sleep 1
pick_category Admission
d set "$D" Headroom 30
sleep 1
d press "$D" OK
sleep 3
step "config dialog: OK with a direct write"
$N command open-dialog
sleep 5
D=$(dialog_title)
pick_category Panel
d press "$D" "Send allotd's changes through the widget"
d press "$D" Apply
sleep 1
pick_category Admission
d set "$D" Headroom 35
sleep 1
d press "$D" OK
sleep 3
step "config dialog: closing with unsaved edits"
$N command open-dialog
sleep 5
D=$(dialog_title)
d set "$D" Headroom 40
sleep 1
d press "$D" Cancel
sleep 2
d windows | grep -F "Allot"
$N shot dialog-cancel-unsaved

step "results"
$N log 'SettingsSpike' | sed 's/^qml: SettingsSpike //'
echo "== server requests"
cat "$ROOT/server.log"
echo "== QML warnings"
$N log 'settingsspike.*(rror|arning)|TypeError|ReferenceError' | head -20
$N stop > /dev/null
$N unserve
pgrep -a -f "wayland-s03|mock-settingsd" || echo "nothing left running"
