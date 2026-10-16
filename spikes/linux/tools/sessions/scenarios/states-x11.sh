# Rows whose fill follows hover, press and keyboard focus, on X11 through XTest: the states
# reach the resolver, a click on a row activates it, a click on its switch flips the switch
# alone, Tab follows the scene's order, Enter and Space activate.
. "$(dirname "$0")/common.sh"
W=$(x11_window states)
xdotool mousemove --window "$W" 150 65
shot states-x11-hover 0.3
pixel states-x11-hover 150 70 dbeafe
xdotool mousedown 1
shot states-x11-pressed 0.3
pixel states-x11-pressed 150 70 93c5fd
xdotool mouseup 1
sleep 0.2
count "[activate] Layout(Box)" 1 "a click on a row activates it"
xdotool mousemove --window "$W" 430 65 click 1
sleep 0.3
expect "[app] switch 1 true" "a click on the row's switch flips the switch"
count "[activate] Layout(Box)" 1 "the switch owns its press: the row does not activate"
xdotool mousemove --window "$W" 150 400 key Tab
shot states-x11-focus 0.3
pixel states-x11-focus 150 126 fde68a
xdotool key Tab Tab
shot states-x11-focus3 0.3
pixel states-x11-focus3 150 182 fde68a
xdotool key Return
sleep 0.2
xdotool key space
sleep 0.3
count "[activate] Layout(Box)" 3 "Enter and Space activate the focused row"
