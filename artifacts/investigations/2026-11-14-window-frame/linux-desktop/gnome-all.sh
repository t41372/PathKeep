#!/bin/bash
# Runs gnomecheck.sh for GNOME Wayland and X11, before (main) and after (fix), each in its own session bus.
rm -f /home/boxd/gnome-all.CHECK_DONE
for spec in "wayland pk-main gnome-wl-before" "wayland pk-fix gnome-wl-after" "x11 pk-main gnome-x11-before" "x11 pk-fix gnome-x11-after"; do
  set -- $spec
  timeout 1500 dbus-run-session -- bash /home/boxd/gnomecheck.sh "$1" "/home/boxd/$2" "$3"
done
pkill -f gnome-shell; pkill -f 'Xvfb :99'
touch /home/boxd/gnome-all.CHECK_DONE
