# The runtime on the loop: a spawned task sleeping and ticking, a scheduled action, a resource
# loading and reloading.
. "$(dirname "$0")/common.sh"
sleep 1.2
grep -E "^(start|tick|scheduled|loaded)" "$LOG" | sed "s/^/INFO $KIND: /"
expect "tick 5 at" "a spawned task sleeps and ticks five times"
expect "scheduled at" "a scheduled action fires"
expect "loaded 1 at" "a resource loads"
expect "loaded 2 at" "a resource loads again when its source changes"
