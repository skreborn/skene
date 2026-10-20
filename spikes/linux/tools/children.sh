# Signals a script's own helpers, never a process that took over a helper's PID after the
# helper ended.
#
#   . tools/children.sh
#   signal_children <signal> <pid...>   signals each PID still a child of this shell
#   is_child <pid>                      whether the PID is still a child of this shell
#
# A child counts whether the script's main shell ($$) or the subshell calling ($BASHPID)
# started it.

is_child() {
  local parent
  parent=$(ps -o ppid= -p "$1" 2>/dev/null | tr -d ' ')
  [ -n "$parent" ] && { [ "$parent" = "$$" ] || [ "$parent" = "$BASHPID" ]; }
}

signal_children() {
  local signal=$1 pid
  shift
  for pid in "$@"; do
    if is_child "$pid"; then kill "-$signal" "$pid" 2>/dev/null; fi
  done
}
