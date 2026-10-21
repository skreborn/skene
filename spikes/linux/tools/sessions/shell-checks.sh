#!/usr/bin/env bash
# Runs every shell scenario, each in a private session (tools/sessions/session.sh inside
# dbus-run-session), and prints their PASS, FAIL, INFO and FRAMES lines.
#
#   shell-checks.sh <debug apps dir> <release apps dir> <results dir>
#
# TOOLS (the drivers, the portal stand-in, the XSETTINGS manager, the clipboard client) is
# tools/bin. ONLY=<scenario name> runs one.
set -u
here=$(cd "$(dirname "$0")" && pwd)
debug=$1 release=$2 results=$3
export TOOLS=${TOOLS:-$here/../bin} CHECK="python3 -I $here/check.py"
scenarios=$here/scenarios
mkdir -p "$results"

run() { # run <name> <kind> <app> <scenario> [VAR=value ...]
  local name=$1 kind=$2 app=$3 scenario=$4
  shift 4
  if [ -n "${ONLY:-}" ] && [ "$ONLY" != "$name" ]; then return; fi
  rm -rf "${results:?}/$name"
  mkdir -p "$results/$name"
  env "$@" OUT="$results/$name" APP="$app" timeout 180 dbus-run-session -- \
    "$here/session.sh" "$kind" bash "$scenarios/$scenario" >"$results/$name/session.log" 2>&1
  grep -E "^(PASS|FAIL|INFO|FRAMES|TIMING)" "$results/$name/session.log" | sed "s|^|[$name] |"
}

# Rendering, resizing, scale, chrome.
run hello-x11 x11 "$release/hello" hello-x11.sh
run hello-sway sway "$release/hello" hello-sway.sh
run hello-mutter mutter "$release/hello" hello-mutter.sh
run hello-weston weston "$release/hello" hello-weston.sh
run hello-kwin kwin "$release/hello" hello-kwin.sh
# States feeding the resolver, activation, focus order.
run states-x11 x11 "$debug/states" states-x11.sh
run states-sway sway "$debug/states" states-wayland.sh DRIVER=sway SWAY_FLOAT=1 SWAY_BORDER=none
run states-mutter mutter "$debug/states" states-wayland.sh DRIVER=mutter-dbus DX=24 DY=62 HEADER=38
# The reader's environment.
run environment-sway sway "$debug/environment" environment.sh SKENE_SIZE=480x400 SWAY_BORDER=none ACCENT_X=1225 ACCENT_Y=128 \
  "PORTAL=2 0 color-scheme=1@1 contrast=1@2 accent-color=0.9,0.2,0.3@3 reduced-motion=1@3 color-scheme=2@4 contrast=0@4"
run environment-mutter mutter "$debug/environment" environment.sh SKENE_SIZE=480x400 DX=24 DY=62 ACCENT_X=449 ACCENT_Y=190 \
  "PORTAL=2 0 color-scheme=1@1 contrast=1@2 accent-color=0.9,0.2,0.3@3 reduced-motion=1@3 color-scheme=2@4 contrast=0@4"
run xsettings-x11 x11 "$debug/environment" xsettings-x11.sh \
  "BEFORE=$TOOLS/xsettings-manager Xft/DPI=147456 Net/ThemeName=Adwaita-dark Gtk/EnableAnimations=0 @ Xft/DPI=98304 Net/ThemeName=HighContrast"
# The clipboard and the primary selection, the keyboard, the runtime.
run clipboard-x11 x11 "$debug/clipboard" clipboard.sh CLIP=x11
run clipboard-sway sway "$debug/clipboard" clipboard.sh CLIP=wlr DRIVER=sway
run keys-x11 x11 "$debug/states" keys-x11.sh
run repeat-sway sway "$debug/states" repeat-sway.sh
run runtime-x11 x11 "$debug/runtime" runtime-x11.sh
# Frame costs, release builds.
run frames-x11 x11 "$release/hello" frames.sh
run frames-sway sway "$release/hello" frames.sh DRIVER=sway SWAY_BORDER=none
run frames-mutter mutter "$release/hello" frames.sh DRIVER=mutter-dbus HEADER=38
run rows-x11 x11 "$release/rows" rows.sh SKENE_SIZE=800x900
run rows-sway sway "$release/rows" rows.sh DRIVER=sway SKENE_SIZE=800x900 SWAY_BORDER=none MONITOR=800x900
run rows-mutter mutter "$release/rows" rows.sh DRIVER=mutter-dbus SKENE_SIZE=800x900 HEADER=38
