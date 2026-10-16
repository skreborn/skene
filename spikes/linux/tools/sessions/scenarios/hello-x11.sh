# Hello on X11: it renders; the Dark switch reinks the page; a resize past 900 pixels relays
# the content out under its wider padding.
. "$(dirname "$0")/common.sh"
W=$(x11_window hello)
shot hello-x11
size hello-x11 480 640
pixel hello-x11 30 100 ffffff
pixel hello-x11 4 4 f2f2f7
xdotool mousemove --window "$W" 418 259 click 1
shot hello-x11-dark
pixel hello-x11-dark 4 4 000000
xdotool mousemove --window "$W" 418 259 click 1
shot hello-x11-light
pixel hello-x11-light 4 4 f2f2f7
xdotool windowsize "$W" 1000 700
shot hello-x11-wide 0.6
size hello-x11-wide 1000 700
pixel hello-x11-wide 30 100 f2f2f7
pixel hello-x11-wide 60 100 ffffff
expect "[x11] configure 1000x700" "a resize reaches the shell"
expect "present shm" "frames go through MIT-SHM"
