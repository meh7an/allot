#!/usr/bin/env bash
# Run by the nested KWin: tells D-Bus-activated services about the display, notes the session's
# D-Bus address, then starts what its Plasma shell needs and the shell itself.
export QT_QPA_PLATFORM=wayland
dbus-update-activation-environment WAYLAND_DISPLAY XDG_RUNTIME_DIR QT_QPA_PLATFORM HOME PATH \
  XDG_CURRENT_DESKTOP
printf '%s' "$DBUS_SESSION_BUS_ADDRESS" > "$XDG_RUNTIME_DIR/session.env"
/usr/lib/kactivitymanagerd &
sleep 1
exec plasmashell --no-respawn
