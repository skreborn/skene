#!/usr/bin/env bash
# One private session per call, run inside dbus-run-session (its own session bus): a display
# (x11: Xvfb; x11-wm: Xvfb + mutter --x11 as the window manager; sway: headless sway with
# pixman; mutter: headless mutter with PipeWire for its RemoteDesktop streams; weston: weston on
# its X11 backend in a private Xvfb, which XTest drives (DISPLAY names that Xvfb); kwin: KWin's
# virtual backend, its fake input open, $SKENE_KWIN naming the kwin_wayland to run, as it does
# for skene-runner), optionally the
# settings-portal stand-in (PORTAL="<scheme> <contrast> [changes]"),
# BEFORE (a helper started in the background first, an XSETTINGS manager), the app ($APP with
# $APP_ARGS, SKENE_TRACE on, SKENE_SHOT and SKENE_PROBE in $OUT), then
# "$@" with the session's environment, then teardown.
#
#   dbus-run-session -- tools/sessions/session.sh <kind> <command...>
#
# OUT names the log directory. APP=none starts no app; APP_DELAY waits that many seconds before
# starting it, the session settled. MONITOR sets the virtual monitor's size
# (mutter), SCALE the output's scale (sway), SWAY_FLOAT=1 floats the app's window (sway). Display numbers are 400-699, sockets unique.
set -u
here=$(cd "$(dirname "$0")" && pwd)
KIND=$1; shift
OUT=${OUT:?OUT names the log directory}
mkdir -p "$OUT"
T0=$(date +%s%N)
ms() { echo $(( ($(date +%s%N) - T0) / 1000000 )); }
export XDG_RUNTIME_DIR=$(mktemp -d /tmp/skene-shell-XXXXXX)
chmod 700 "$XDG_RUNTIME_DIR"
# The private-session marker the input tools require before they inject anything.
export SKENE_PRIVATE_SESSION=$XDG_RUNTIME_DIR
export SKENE_PROBE=$OUT/probe SKENE_SHOT=$OUT/frame.png SKENE_TRACE=1 LANG=C.UTF-8
MONITOR=${MONITOR:-1280x1024}
unset DISPLAY WAYLAND_DISPLAY SKENE_BACKEND
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null; done
  wait 2>/dev/null
  rm -rf "$XDG_RUNTIME_DIR"
}
trap cleanup EXIT
case $KIND in
  x11|x11-wm)
    for _ in $(seq 40); do
      N=$((400 + RANDOM % 300)); [ -e /tmp/.X$N-lock ] || break
    done
    Xvfb ":$N" -screen 0 ${SCREEN:-1280x1024x24} -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
    PIDS+=($!)
    export DISPLAY=:$N
    for _ in $(seq 100); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.02; done
    if [ "$KIND" = x11-wm ]; then
      mutter --x11 --replace --sm-disable >"$OUT/wm.log" 2>&1 &
      PIDS+=($!)
      for _ in $(seq 100); do xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q "window id" && break; sleep 0.05; done
    fi
    export SKENE_BACKEND=x11
    ;;
  sway)
    cat >"$XDG_RUNTIME_DIR/config" <<CONF
output HEADLESS-1 resolution ${MONITOR:-1280x1024} position 0 0 scale ${SCALE:-1}
default_border ${SWAY_BORDER:-normal}
CONF
    if [ "${SWAY_FLOAT:-0}" = 1 ]; then
      echo 'for_window [app_id="^dev[.]skene[.]"] floating enable' >>"$XDG_RUNTIME_DIR/config"
    fi
    WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
      sway --unsupported-gpu -c "$XDG_RUNTIME_DIR/config" >"$OUT/sway.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 200); do ls "$XDG_RUNTIME_DIR"/wayland-* >/dev/null 2>&1 && break; sleep 0.02; done
    export WAYLAND_DISPLAY=$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-* | grep -v lock | head -1)")
    export SWAYSOCK=$(ls "$XDG_RUNTIME_DIR"/sway-ipc.* 2>/dev/null | head -1)
    export SKENE_BACKEND=wayland
    ;;
  mutter)
    pipewire >"$OUT/pipewire.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/pipewire-0" ] && break; sleep 0.02; done
    SOCKET=skene-shell-$$
    mutter --headless --wayland --no-x11 --wayland-display "$SOCKET" \
      --virtual-monitor ${MONITOR:-1280x1024} >"$OUT/mutter.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 200); do [ -S "$XDG_RUNTIME_DIR/$SOCKET" ] && break; sleep 0.02; done
    export WAYLAND_DISPLAY=$SOCKET
    for _ in $(seq 200); do
      gdbus introspect --session --dest org.gnome.Mutter.RemoteDesktop \
        --object-path /org/gnome/Mutter/RemoteDesktop >/dev/null 2>&1 && break
      sleep 0.02
    done
    export SKENE_BACKEND=wayland
    ;;
  weston)
    for _ in $(seq 40); do
      N=$((400 + RANDOM % 300)); [ -e /tmp/.X$N-lock ] || break
    done
    Xvfb ":$N" -screen 0 ${MONITOR:-1280x1024}x24 -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
    PIDS+=($!)
    export DISPLAY=:$N
    for _ in $(seq 100); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.02; done
    weston --backend=x11 --renderer=pixman --idle-time=0 --socket=skene-shell-$$ \
      --width="${MONITOR%x*}" --height="${MONITOR#*x}" >"$OUT/weston.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 200); do [ -S "$XDG_RUNTIME_DIR/skene-shell-$$" ] && break; sleep 0.02; done
    export WAYLAND_DISPLAY=skene-shell-$$
    export SKENE_BACKEND=wayland
    ;;
  kwin)
    KWIN_WAYLAND_NO_PERMISSION_CHECKS=1 KWIN_COMPOSE=Q QT_QPA_PLATFORM=offscreen \
      "${SKENE_KWIN:-kwin_wayland}" --virtual --socket skene-shell-$$ --width "${MONITOR%x*}" \
      --height "${MONITOR#*x}" >"$OUT/kwin.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 200); do [ -S "$XDG_RUNTIME_DIR/skene-shell-$$" ] && break; sleep 0.02; done
    export WAYLAND_DISPLAY=skene-shell-$$
    export SKENE_BACKEND=wayland
    ;;
esac
echo "TIMING $KIND display ready $(ms) ms"
if [ -n "${PORTAL:-}" ]; then
  # shellcheck disable=SC2086
  "$here/../bin/portal-stub" $PORTAL >"$OUT/portal.log" 2>&1 &
  PIDS+=($!)
  for _ in $(seq 200); do
    gdbus introspect --session --dest org.freedesktop.portal.Desktop \
      --object-path /org/freedesktop/portal/desktop >/dev/null 2>&1 && break
    sleep 0.02
  done
  echo "TIMING $KIND portal ready $(ms) ms"
fi
if [ -n "${BEFORE:-}" ]; then
  # A helper the app finds at start (an XSETTINGS manager): run in the background.
  bash -c "$BEFORE" >"$OUT/before.log" 2>&1 &
  PIDS+=($!)
  sleep 0.3
fi
if [ "${APP:-none}" != none ]; then
  sleep "${APP_DELAY:-0}"
  # shellcheck disable=SC2086
  A0=$(date +%s%N)
  "$APP" ${APP_ARGS:-} >"$OUT/app.log" 2>&1 &
  APP_PID=$!
  PIDS+=($APP_PID)
  export APP_PID
  for _ in $(seq 500); do grep -q "^\[frame\]" "$OUT/app.log" 2>/dev/null && break; sleep 0.01; done
  echo "TIMING $KIND first frame $(ms) ms"
  echo "TIMING $KIND app start to first frame $(( ($(date +%s%N) - A0) / 1000000 )) ms"
  if [ "$KIND" = sway ]; then
    # sway places the window after its first frame: the driver aims once it is there.
    for _ in $(seq 300); do grep -q "^\[wayland\] configure .*ACTIVATED" "$OUT/app.log" 2>/dev/null && break; sleep 0.01; done
    sleep 0.2
  fi
fi
export OUT KIND
"$@"
STATUS=$?
echo "TIMING $KIND session done $(ms) ms"
exit $STATUS
