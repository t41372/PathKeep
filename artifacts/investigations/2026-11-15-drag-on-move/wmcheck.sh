#!/bin/bash
exec > /home/boxd/wmcheck.log 2>&1
pkill -f stress-ng; pkill -f mutter; pkill xfwm4; pkill -f Xvfb; sleep 1
Xvfb :77 -screen 0 1920x1200x24 >/dev/null 2>&1 &
sleep 2; export DISPLAY=:77
mutter --x11 --replace >/dev/null 2>&1 &
sleep 4; echo "1:"; wmctrl -m
xfwm4 --replace --compositor=off >/dev/null 2>&1 &
sleep 4; echo "2:"; wmctrl -m
mutter --x11 --replace >/dev/null 2>&1 &
sleep 4; echo "3:"; wmctrl -m
pgrep -a xfwm4; pgrep -a mutter
pkill -f mutter; pkill xfwm4; pkill -f "Xvfb :77"
echo END
