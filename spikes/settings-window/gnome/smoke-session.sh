#!/bin/bash
# The M0.3 smoke run inside dbus-run-session: see smoke.sh.
set -u
WINDOW="Allot Settings Spike"
D="python3 $SPIKE/linux/atspi-drive.py"

gsettings set org.gnome.shell disable-user-extensions false
gsettings reset org.gnome.shell disabled-extensions
gsettings set org.gnome.shell enabled-extensions "['$UUID', 'allot-s03-helper@mehran.tech']"
gsettings set org.gnome.desktop.interface toolkit-accessibility true
gnome-shell --headless --no-x11 --virtual-monitor 1280x900 > "$HOME/gnome-shell.log" 2>&1 &
shell=$!

ev() {
    busctl --user call org.gnome.Shell /org/gnome/Shell org.gnome.Shell Eval s \
        "const spike = Main.extensionManager.lookup('$UUID').stateObj;
$1" 2>&1 | sed 's/\\n/\n/g; s/\\"/"/g'
}
shot() {
    busctl --user call org.gnome.Shell.Screenshot /org/gnome/Shell/Screenshot \
        org.gnome.Shell.Screenshot Screenshot bbs false false "$SHOTS/$1.png" > /dev/null 2>&1
    echo "shot $1"
}
step() {
    echo
    echo "== $1"
    echo "{\"step\": \"$1\"}" >> "$HOME/settings-spike.log"
}
hand_edit() {
    curl -s -X POST "http://127.0.0.1:$PORT/spike/hand-edit" > /dev/null
}
mode() {
    echo "$1" > "$HOME/settings-spike-mode"
}
# Closes it as its title bar X would.
close_window() {
    ev "global.get_window_actors().map((a) => a.meta_window)
        .find((w) => w.get_title() === '$WINDOW')?.delete(global.get_current_time()); 'closed'"
}
open_again() {
    close_window
    sleep 1.5
    ev "spike.openSettings('smoke'); 'asked'"
    sleep 3
}

for _ in $(seq 40); do
    ev 'global.context.unsafe_mode' | grep -q '"true"' && break
    sleep 1
done
sleep 3
ev "Main.overview.hide(); 'overview hidden'"
ev "const e = Main.extensionManager.lookup('$UUID'); JSON.stringify({ state: e?.state, error: String(e?.error ?? '') })"
sleep 1

step "open from the panel menu"
ev "spike.openSettings('smoke'); 'asked'"
sleep 4
$D windows
$D dump "$WINDOW" > "$SHOTS/dump-spin-rows.txt"
shot open
step "numbers as action rows with a spin button, which AT-SPI can drive"
echo '{"spin_rows": false}' > "$HOME/.allot/settings-spike.json"
open_again
$D dump "$WINDOW" > "$SHOTS/dump-action-rows.txt"
if [ "${PROBE:-}" = 1 ]; then
    kill "$shell"
    wait "$shell"
    exit
fi

step "a second plain openPreferences while open"
ev "spike.openPlain('smoke-second'); 'asked'"
sleep 2
step "a second open that raises"
ev "spike.openSettings('smoke-raise'); 'asked'"
sleep 2

step "edit headroom to 20"
$D set "$WINDOW" Headroom 20
sleep 2
step "headroom 100 is refused and goes back"
$D set "$WINDOW" Headroom 100
sleep 2
$D value "$WINDOW" Headroom
shot invalid
step "a hand edit, then an edit: conflict"
hand_edit
$D set "$WINDOW" "Estimate margin" 12
sleep 2
shot conflict
$D dump "$WINDOW" > "$SHOTS/dump-conflict.txt"
$D press "$WINDOW" "Apply Mine"
sleep 2
$D value "$WINDOW" "Estimate margin"

step "port needs a restart"
$D press "$WINDOW" "Service"
sleep 1
$D set "$WINDOW" Port 47999
sleep 2
$D dump "$WINDOW" | grep -iE "port|restart|reset"
shot restart
step "reset the port"
$D press "$WINDOW" "Reset to default"
sleep 2
$D value "$WINDOW" Port

step "panel setting seen by the shell"
$D press "$WINDOW" "Panel"
sleep 1
$D press "$WINDOW" "Show CPU"
sleep 2
ev "spike._panelLine.label.text"
cat "$HOME/.allot/settings-spike.json"

step "allotd down, then Retry"
mode down
$D press "$WINDOW" "Admission"
sleep 1
$D press "$WINDOW" Retry
sleep 3
shot down
mode unauthorized
step "token refused, then Retry"
$D press "$WINDOW" Retry
sleep 3
$D dump "$WINDOW" | grep -iE "answering|refused|connected" | head -5
mode normal
$D press "$WINDOW" Retry
sleep 2

step "an edit just before closing"
$D set "$WINDOW" Headroom 33
close_window
sleep 3
ev "spike.openSettings('smoke-reopen'); 'asked'"
sleep 3
$D value "$WINDOW" Headroom

step "slow allotd, window does not wait"
mode slow
open_again
step "slow allotd, window waits"
$D press "$WINDOW" "Panel"
sleep 1
$D press "$WINDOW" "Wait for allotd before showing"
sleep 1
open_again
sleep 3
mode normal

step "destroy() instead of close()"
$D press "$WINDOW" "Spike"
sleep 1
$D press "$WINDOW" "Close with destroy() (the shell says this shows an error page)"
sleep 2
$D windows
shot destroy

ev "global.get_window_actors().map((a) => a.meta_window.get_title()).join(' | ')"
kill "$shell" 2> /dev/null
wait "$shell"
