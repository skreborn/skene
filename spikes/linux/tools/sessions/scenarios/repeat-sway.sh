# A held key repeats on Wayland at the compositor's rate after its delay (sway's defaults: 25
# a second after 600 ms): about 10 repeats in a second's hold.
. "$(dirname "$0")/common.sh"
"$TOOLS/drive" sway key Shift_L sleep 100 keydown a sleep 1000 keyup a sleep 200 >/dev/null 2>&1
repeats=$(grep -cF '[key] "a" "KeyA" down repeat' "$LOG")
echo "INFO $KIND: $repeats repeats in a 1000 ms hold"
if [ "$repeats" -ge 8 ] && [ "$repeats" -le 11 ]; then pass "$KIND: a held key repeats ($repeats in 1 s)"; else fail "$KIND: a held key repeats ($repeats in 1 s)"; fi
