#!/usr/bin/env bash
# Every controls check, each in a private session (tools/controls/session.sh under its own
# dbus-run-session): the controls and fields scenarios on Xvfb, headless sway and headless
# mutter; Hangul and a secure field through IBus, Fcitx5 and XIM on X11, gnome-shell's IBus over
# text-input-v3 and Fcitx5 behind sway; the input method choice; the reader probes against
# GtkEntry.
#
#   checks.sh <controls probe binary> <results dir>
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
probe=$1
results=$2
mkdir -p "$results"
cd "$root" || exit 1

session() {
  # session <label> <kind> <IME> <APP_ENV> <command...>
  local label=$1 kind=$2 ime=$3 app_env=$4
  shift 4
  rm -rf "${results:?}/${label:?}"
  LABEL=$label IME=$ime APP=${APP:-$probe} APP_ENV=$app_env OUT=$results/$label \
    timeout 400 dbus-run-session -- tools/controls/session.sh "$kind" "$@" \
    >"$results/$label.txt" 2>&1
  grep -E "^\[$label\] (PASS|FAIL|NOTE|DONE)" "$results/$label.txt"
}

for kind in x11 sway mutter; do
  session "$kind-controls" "$kind" none SKENE_SIZE=520x900 python3.12 -I tools/controls/harness.py controls
  session "$kind-fields" "$kind" none SKENE_SIZE=520x900 python3.12 -I tools/controls/harness.py fields
done
session x11-ibus x11 ibus PROBE_FIELDS_ONLY=1 python3.12 -I tools/controls/harness.py hangul secure
session x11-fcitx x11 fcitx PROBE_FIELDS_ONLY=1 python3.12 -I tools/controls/harness.py hangul secure
session x11-xim x11 ibus "PROBE_FIELDS_ONLY=1 SKENE_IME=xim" \
  python3.12 -I tools/controls/harness.py hangul secure
session gnome-ibus gnome ibus PROBE_FIELDS_ONLY=1 python3.12 -I tools/controls/harness.py hangul secure
session sway-fcitx sway fcitx PROBE_FIELDS_ONLY=1 python3.12 -I tools/controls/harness.py hangul secure

export PROBE=$probe
APP=none session detect-ibus x11 ibus "" tools/controls/detect.sh ibus \
  "GTK_IM_MODULE=xim=>xim" "XMODIFIERS=@im=ibus=>ibus" "QT_IM_MODULE=ibus=>ibus" \
  "SKENE_IME=none=>none"
APP=none session detect-fcitx x11 fcitx "" tools/controls/detect.sh fcitx \
  "GTK_IM_MODULE=fcitx=>fcitx" "QT_IM_MODULE=fcitx5=>fcitx" "XMODIFIERS=@im=fcitx=>fcitx" \
  "GTK_IM_MODULE=ibus=>ibus"
APP=none session detect-none x11 none "" tools/controls/detect.sh none \
  "XMODIFIERS=@im=foo=>xim" "GTK_IM_MODULE=xim=>xim"

# What a reader can ask of the field, the drawn one against GtkEntry (GTK 4.14).
session reader-skene x11 none PROBE_READER=1 python3.12 -I tools/controls/harness.py reader
ATSPI_WINDOW="GTK reference" APP=$root/tools/controls/gtk-ref.py \
  session reader-gtk x11 none "" python3.12 -I tools/controls/harness.py reader

# Orca 46's speech over the same keys: the drawn field, switch and button against GtkEntry,
# GtkSwitch and GtkButton.
orca() {
  local label=$1
  rm -rf "${results:?}/${label:?}"
  APP=none OUT=$results/$label timeout 400 dbus-run-session -- tools/controls/session.sh x11 \
    tools/controls/orca/orca.sh >"$results/$label.txt" 2>&1
}
ORCA_APP=$probe ORCA_APP_ENV="PROBE_READER=1 SKENE_SIZE=480x320" ORCA_WINDOW=controls \
  orca orca-skene
ORCA_APP=$root/tools/controls/gtk-ref.py ORCA_WINDOW="GTK reference" orca orca-gtk
diff "$results/orca-skene/spoken.log" "$results/orca-gtk/spoken.log" >"$results/orca.diff"
echo "[orca] spoken lines: skene $(grep -vc '^###' "$results/orca-skene/spoken.log"), gtk" \
  "$(grep -vc '^###' "$results/orca-gtk/spoken.log"); differing hunks:" \
  "$(grep -c '^[0-9]' "$results/orca.diff") (orca.diff)"
for step in "Tab to the switch" "Space: switch on" "Tab to the button"; do
  skene=$(sed -n "/^### $step\$/,/^###/p" "$results/orca-skene/spoken.log" | grep -v '^###' | tr '\n' ' ')
  gtk=$(sed -n "/^### $step\$/,/^###/p" "$results/orca-gtk/spoken.log" | grep -v '^###' | tr '\n' ' ')
  if [ "$skene" = "$gtk" ]; then
    echo "[orca] PASS $step: skene and GTK both say '$skene'"
  else
    echo "[orca] FAIL $step: skene '$skene', GTK '$gtk'"
  fi
done
