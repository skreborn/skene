# Sourced: a private speech-dispatcher whose only output module is sd_generic appending each
# utterance to $DISPATCHED, and Orca 46 (default settings, its own prefs dir, debug log) on it.
# $SPOKEN is what Orca said, each utterance as its debug log records it (`SPEECH OUTPUT`),
# under the step it followed: the dispatcher drops an utterance of blanks alone now and then.
# Needs B (this directory), OUT (the run's log dir) and tools/children.sh sourced.
SPOKEN=$OUT/spoken.log
DISPATCHED=$OUT/dispatched.log
STEPS=$OUT/steps.log
start_speech() {
  mkdir -p "$OUT/home" "$OUT/sd/modules" "$OUT/prefs"
  : >"$SPOKEN"
  : >"$DISPATCHED"
  : >"$STEPS"
  local port=$((20000 + RANDOM % 20000))
  cat >"$OUT/sd/speechd.conf" <<CONF
LogLevel 3
AudioOutputMethod "libao"
AddModule "log" "sd_generic" "log-generic.conf"
DefaultModule log
LanguageDefaultModule "en" "log"
CONF
  cat >"$OUT/sd/modules/log-generic.conf" <<CONF
GenericExecuteSynth "printf '%s\\\\n' \\'\$DATA\\' >> $DISPATCHED"
AddVoice "en" "MALE1" "default"
GenericLanguage "en" "en" "utf-8"
CONF
  echo "default_driver=null" >"$OUT/home/.libao"
  HOME=$OUT/home speech-dispatcher -s -C "$OUT/sd" -L "$OUT" -P "$OUT/sd.pid" -c inet_socket \
    -p "$port" -t 0 &
  SPEECHD=$!
  export SPEECHD_ADDRESS=inet_socket:127.0.0.1:$port
}
start_orca() {
  HOME=$OUT/home python3.12 "$B/orca-launch.py" -u "$OUT/prefs" \
    --debug-file "$OUT/orca-debug.log" >"$OUT/orca.log" 2>&1 &
  ORCA=$!
  for _ in $(seq 100); do grep -qv '^###' "$DISPATCHED" 2>/dev/null && break; sleep 0.1; done
  sleep 2
}
stop_speech() {
  grep -a "SPEECH OUTPUT" "$OUT/orca-debug.log" >"$OUT/orca-speech.log"
  python3 -I - "$STEPS" "$OUT/orca-speech.log" >"$SPOKEN" <<'MERGE'
import re, sys
steps = [line.rstrip("\n").split(" ", 1) for line in open(sys.argv[1])]
said = []
for line in open(sys.argv[2], errors="replace"):
  match = re.match(r"(\S+) - SPEECH OUTPUT: '(.*)' \{", line)
  if match:
    said.append((match.group(1), match.group(2)))
def seconds(at):
  hours, minutes, rest = at.split(":")
  return int(hours) * 3600 + int(minutes) * 60 + float(rest)
events = [(seconds(at), 0, text) for at, text in steps]
events += [(seconds(at), 1, text) for at, text in said]
start = min((at for at, _, _ in events), default=0)
# A run that crosses midnight: what is half a day before its start is the next day.
events = [(at + 86400 if at < start - 43200 else at, order, text) for at, order, text in events]
for _, _, text in sorted(events, key=lambda event: (event[0], event[1])):
  print(text)
MERGE
  signal_children TERM "$ORCA"
  for _ in $(seq 30); do is_child "$ORCA" || break; sleep 0.1; done
  signal_children KILL "$ORCA"
  wait "$ORCA" 2>/dev/null
  signal_children TERM "$SPEECHD"
}
step() {
  sleep "${PAUSE:-1.2}"
  echo "$(date +%H:%M:%S.%6N) ### $*" >>"$STEPS"
  echo "### $*"
}
