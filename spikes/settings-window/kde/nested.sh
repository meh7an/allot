#!/usr/bin/env bash
# M0.3 on a Plasma 6 machine: a throwaway Plasma session in a nested KWin with its own HOME, D-Bus
# and runtime folder under ~/allot-s03, so the desktop's panel, config and any real allotd are
# untouched. Copy spikes/settings-window to ~/allot-s03/spike first.
#   nested.sh start [window]   start it; `window` shows it in a window on the desktop
#   nested.sh widget           installs the spike widget (run while stopped)
#   nested.sh panel            adds it to the session's panel
#   nested.sh serve            starts the mock server (port 47960) with a test token
#   nested.sh mode <mode>      normal, slow, down or unauthorized
#   nested.sh hand-edit        changes a value as an edit by hand would
#   nested.sh command <name>   open-dialog, open-window, close-window or expand, for the widget
#   nested.sh drive <args...>  linux/atspi-drive.py inside the session
#   nested.sh a11y             turns the session's accessibility bus on, for drive
#   nested.sh run <command...> runs a command inside it
#   nested.sh shot <name>      a screenshot into ~/allot-s03/shots
#   nested.sh log [pattern]    the session's log, filtered
#   nested.sh unserve, stop    stops the mock server, the session
set -u
ROOT="$HOME/allot-s03"
SPIKE="$ROOT/spike"
FAKE_HOME="$ROOT/home"
RUN="$ROOT/run"
SOCKET=wayland-s03
LOG="$ROOT/nested.log"
ENV_FILE="$RUN/session.env"
PORT=47960
WIDGET=tech.mehran.allot.settingsspike

inside() {
  env -i HOME="$FAKE_HOME" USER="$USER" LANG="${LANG:-en_US.UTF-8}" \
    PATH="/usr/local/bin:/usr/bin:/bin" XDG_RUNTIME_DIR="$RUN" \
    WAYLAND_DISPLAY="$SOCKET" QT_QPA_PLATFORM=wayland XDG_SESSION_TYPE=wayland \
    XDG_CURRENT_DESKTOP=KDE KDE_FULL_SESSION=true KDE_SESSION_VERSION=6 \
    DBUS_SESSION_BUS_ADDRESS="$(cat "$ENV_FILE")" "$@"
}

case "${1:-}" in
  start)
    mkdir -p "$FAKE_HOME/.allot" "$ROOT/shots"
    mkdir -p -m 700 "$RUN"
    rm -f "$ENV_FILE" "$RUN/$SOCKET" "$RUN/$SOCKET.lock"
    args=(--virtual --width 1280 --height 900)
    host_display=()
    if [ "${2:-}" = window ]; then
      args=(--width 1280 --height 900)
      real_run="/run/user/$(id -u)"
      desktop="$(ls "$real_run" | grep -E '^wayland-[0-9]+$' | head -1)"
      host_display=(--wayland-display "$real_run/${WAYLAND_DISPLAY:-$desktop}")
    fi
    env -i HOME="$FAKE_HOME" USER="$USER" LANG="${LANG:-en_US.UTF-8}" \
      PATH="/usr/local/bin:/usr/bin:/bin" XDG_RUNTIME_DIR="$RUN" \
      KWIN_WAYLAND_NO_PERMISSION_CHECKS=1 XDG_CURRENT_DESKTOP=KDE KDE_FULL_SESSION=true \
      KDE_SESSION_VERSION=6 XDG_SESSION_TYPE=wayland QT_FORCE_STDERR_LOGGING=1 \
      QT_LOGGING_RULES="js.debug=true;qml.debug=true" \
      setsid dbus-run-session -- kwin_wayland "${args[@]}" "${host_display[@]}" --no-lockscreen \
      --socket "$SOCKET" "$SPIKE/kde/session.sh" > "$LOG" 2>&1 < /dev/null &
    echo $! > "$ROOT/nested.pid"
    for _ in $(seq 1 40); do [ -s "$ENV_FILE" ] && break; sleep 0.5; done
    sleep 8
    pgrep -a -f "$SOCKET" | head -3
    ;;
  widget)
    # Replaced while the session is stopped, since plasmashell keeps the QML it has loaded.
    pkg="$FAKE_HOME/.local/share/plasma/plasmoids"
    mkdir -p "$pkg"
    rm -rf "${pkg:?}/$WIDGET" "$FAKE_HOME/.cache/plasmashell/qmlcache"
    cp -r "$SPIKE/kde/$WIDGET" "$pkg/"
    mkdir -p "$pkg/$WIDGET/contents/code"
    cp "$SPIKE/linux/settings-client.mjs" "$pkg/$WIDGET/contents/code/"
    ;;
  panel)
    inside qdbus6 org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript \
      "var p = panels()[0]; if (p.widgets('$WIDGET').length === 0) p.addWidget('$WIDGET');"
    ;;
  serve)
    mkdir -p "$FAKE_HOME/.allot"
    printf 'spike-token\n' > "$FAKE_HOME/.allot/token"
    echo normal > "$FAKE_HOME/settings-spike-mode"
    HOME="$FAKE_HOME" setsid python3 "$SPIKE/linux/mock-settingsd.py" "$PORT" \
      > "$ROOT/server.log" 2>&1 < /dev/null &
    echo $! > "$ROOT/server.pid"
    ;;
  mode) echo "$2" > "$FAKE_HOME/settings-spike-mode" ;;
  hand-edit) curl -s -X POST "http://127.0.0.1:$PORT/spike/hand-edit" > /dev/null ;;
  command) echo "$2" > "$FAKE_HOME/settings-spike-command" ;;
  drive) shift; inside python3 "$SPIKE/linux/atspi-drive.py" "$@" ;;
  a11y)
    inside gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
      --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled "<true>" > /dev/null
    ;;
  run) shift; inside "$@" ;;
  shot) inside spectacle -b -n -f -o "$ROOT/shots/$2.png" 2>/dev/null; ls "$ROOT/shots/$2.png" ;;
  log) if [ -n "${2:-}" ]; then grep -E "$2" "$LOG" | tail -300; else tail -80 "$LOG"; fi ;;
  unserve)
    [ -f "$ROOT/server.pid" ] && kill "$(cat "$ROOT/server.pid")" 2>/dev/null
    rm -f "$ROOT/server.pid"
    ;;
  stop)
    [ -f "$ROOT/nested.pid" ] && kill -- -"$(cat "$ROOT/nested.pid")" 2>/dev/null
    rm -f "$ROOT/nested.pid" "$ENV_FILE"
    sleep 1
    pgrep -a -f "$SOCKET"; echo stopped
    ;;
  *) sed -n '2,19p' "$0"; exit 2 ;;
esac
