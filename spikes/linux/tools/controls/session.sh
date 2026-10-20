#!/usr/bin/env bash
# One private session for the controls checks, run inside dbus-run-session (its own session
# bus): a private runtime dir, HOME and GSettings keyfile; a display; the AT-SPI bus switched
# on; optionally an input method with its Hangul engine; the app; then "$@"; then teardown.
#
#   dbus-run-session -- tools/controls/session.sh <kind> <command...>
#
# Kinds: x11 (Xvfb), sway (headless sway, pixman), gnome (headless gnome-shell 46: mutter's
# compositor with its IBus bridge, which spawns ibus-daemon itself; a private empty system bus),
# mutter (headless mutter with PipeWire, no input method bridge). XWAYLAND=1 (gnome, mutter):
# the compositor runs Xwayland as a desktop session does, its display exported as DISPLAY.
# IME=ibus|fcitx|none (x11: ibus-daemon or fcitx5 for the X11 clients; sway: fcitx5 through
# input-method-v2; gnome: gnome-shell's IBus, its source Hangul, the US layout under IME=none).
# APP is the app (none for none), with APP_ENV extra variables; OUT the log directory. DRIVE is
# exported: the drive backend.
set -u
here=$(cd "$(dirname "$0")" && pwd)
. "$here/../children.sh"
root=$(cd "$here/../.." && pwd)
KIND=$1; shift
OUT=${OUT:?OUT names the log directory}
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
export OUT KIND
T0=$(date +%s%N)
ms() { echo $(( ($(date +%s%N) - T0) / 1000000 )); }
export XDG_RUNTIME_DIR=$(mktemp -d /tmp/skene-controls-XXXXXX)
chmod 700 "$XDG_RUNTIME_DIR"
# The private-session marker the input tools require before they inject anything.
export SKENE_PRIVATE_SESSION=$XDG_RUNTIME_DIR
export HOME=$XDG_RUNTIME_DIR/home XDG_CONFIG_HOME=$XDG_RUNTIME_DIR/home/.config
export XDG_CACHE_HOME=$XDG_RUNTIME_DIR/home/.cache XDG_DATA_HOME=$XDG_RUNTIME_DIR/home/.local/share
export GSETTINGS_BACKEND=keyfile LANG=C.UTF-8 LC_ALL=C.UTF-8 NO_AT_BRIDGE=0 GTK_A11Y=atspi
mkdir -p "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME"
export SKENE_PROBE=$OUT/probe SKENE_SHOT=$OUT/frame.png SKENE_SHOT_POPUP=$OUT/popup.png SKENE_TRACE=1
unset DISPLAY XAUTHORITY WAYLAND_DISPLAY SKENE_BACKEND GTK_IM_MODULE QT_IM_MODULE XMODIFIERS IBUS_ADDRESS
PIDS=()
cleanup() {
  signal_children TERM "${PIDS[@]}"
  sleep 0.3
  signal_children KILL "${PIDS[@]}"
  wait 2>/dev/null
  rm -rf "$XDG_RUNTIME_DIR"
}
trap cleanup EXIT
IME=${IME:-none}

fcitx_profile() {
  mkdir -p "$XDG_CONFIG_HOME/fcitx5/conf"
  cat >"$XDG_CONFIG_HOME/fcitx5/profile" <<'P'
[Groups/0]
Name=Default
Default Layout=us
DefaultIM=hangul

[Groups/0/Items/0]
Name=keyboard-us
Layout=

[Groups/0/Items/1]
Name=hangul
Layout=

[GroupOrder]
0=Default
P
  cat >"$XDG_CONFIG_HOME/fcitx5/config" <<'P'
[Behavior]
ActiveByDefault=True
ShareInputState=No
P
}

wait_name() {
  for _ in $(seq 150); do
    gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner "$1" 2>/dev/null | grep -q true && return 0
    sleep 0.1
  done
  return 1
}

# The compositor's X11 flag: none under XWAYLAND=1, --no-x11 otherwise.
x11_flag() { [ "${XWAYLAND:-0}" = 1 ] || echo --no-x11; }

# Under XWAYLAND=1, exports as DISPLAY the Xwayland display the compositor logs in $1, and as
# XAUTHORITY the cookie it writes for it.
xwayland_display() {
  [ "${XWAYLAND:-0}" = 1 ] || return 0
  for _ in $(seq 100); do grep -q "Using public X11 display" "$1" && break; sleep 0.1; done
  DISPLAY=$(sed -n 's/.*Using public X11 display \(:[0-9]*\).*/\1/p' "$1" | head -1)
  XAUTHORITY=$(ls "$XDG_RUNTIME_DIR"/.mutter-Xwaylandauth.* 2>/dev/null | head -1)
  if [ -n "$DISPLAY" ] && [ -n "$XAUTHORITY" ]; then
    export DISPLAY XAUTHORITY
  else
    unset DISPLAY XAUTHORITY
    echo "NOTE the compositor names no Xwayland display"
  fi
}

case $KIND in
  x11)
    for _ in $(seq 40); do
      N=$((400 + RANDOM % 300)); [ -e /tmp/.X$N-lock ] || break
    done
    Xvfb ":$N" -screen 0 ${SCREEN:-1280x1024x24} -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
    PIDS+=($!)
    export DISPLAY=:$N
    for _ in $(seq 100); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.02; done
    export SKENE_BACKEND=x11 DRIVE=x11
    ;;
  sway)
    cat >"$XDG_RUNTIME_DIR/config" <<CONF
output HEADLESS-1 resolution ${MONITOR:-1280x1024} position 0 0 scale ${SCALE:-1}
default_border none
CONF
    WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
      sway --unsupported-gpu -c "$XDG_RUNTIME_DIR/config" >"$OUT/sway.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 200); do ls "$XDG_RUNTIME_DIR"/wayland-* >/dev/null 2>&1 && break; sleep 0.02; done
    export WAYLAND_DISPLAY=$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-* | grep -v lock | head -1)")
    export SWAYSOCK=$(ls "$XDG_RUNTIME_DIR"/sway-ipc.* 2>/dev/null | head -1)
    export SKENE_BACKEND=wayland DRIVE=sway
    ;;
  gnome)
    pipewire >"$OUT/pipewire.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/pipewire-0" ] && break; sleep 0.02; done
    cat >"$XDG_RUNTIME_DIR/system.conf" <<CONF
<busconfig>
  <type>system</type>
  <listen>unix:path=$XDG_RUNTIME_DIR/system_bus_socket</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow user="*"/><allow own="*"/><allow send_destination="*"/><allow receive_sender="*"/>
  </policy>
</busconfig>
CONF
    dbus-daemon --config-file="$XDG_RUNTIME_DIR/system.conf" --nofork --nopidfile >"$OUT/system-bus.log" 2>&1 &
    PIDS+=($!)
    export DBUS_SYSTEM_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/system_bus_socket
    for _ in $(seq 50); do [ -S "$XDG_RUNTIME_DIR/system_bus_socket" ] && break; sleep 0.1; done
    mkdir -p "$XDG_CONFIG_HOME/glib-2.0/settings"
    if [ "$IME" = none ]; then SOURCES="[('xkb', 'us')]"; else SOURCES="[('ibus', 'hangul')]"; fi
    cat >"$XDG_CONFIG_HOME/glib-2.0/settings/keyfile" <<CONF
[org/gnome/desktop/input-sources]
sources=$SOURCES

[org/freedesktop/ibus/engine/hangul]
initial-input-mode='hangul'

[org/gnome/shell]
welcome-dialog-last-shown-version='99.0'

[org/gnome/desktop/interface]
enable-animations=false

[org/gnome/desktop/notifications]
show-banners=false
CONF
    SOCKET=skene-gnome-$$
    gnome-shell --headless --wayland $(x11_flag) --wayland-display "$SOCKET" \
      --virtual-monitor ${MONITOR:-1280x1024} >"$OUT/gnome-shell.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 300); do [ -S "$XDG_RUNTIME_DIR/$SOCKET" ] && break; sleep 0.1; done
    export WAYLAND_DISPLAY=$SOCKET
    wait_name org.gnome.Mutter.RemoteDesktop
    # The shell takes input once its startup is over (its overview animation included).
    for _ in $(seq 300); do grep -q "GNOME Shell started" "$OUT/gnome-shell.log" && break; sleep 0.1; done
    grep -q "GNOME Shell started" "$OUT/gnome-shell.log" || echo "NOTE gnome-shell did not finish starting"
    xwayland_display "$OUT/gnome-shell.log"
    export SKENE_BACKEND=wayland DRIVE=mutter-dbus
    ;;
  mutter)
    pipewire >"$OUT/pipewire.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/pipewire-0" ] && break; sleep 0.02; done
    SOCKET=skene-controls-$$
    mutter --headless --wayland $(x11_flag) --wayland-display "$SOCKET" \
      --virtual-monitor ${MONITOR:-1280x1024} >"$OUT/mutter.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 200); do [ -S "$XDG_RUNTIME_DIR/$SOCKET" ] && break; sleep 0.02; done
    export WAYLAND_DISPLAY=$SOCKET
    wait_name org.gnome.Mutter.RemoteDesktop
    xwayland_display "$OUT/mutter.log"
    export SKENE_BACKEND=wayland DRIVE=mutter-dbus
    ;;
esac
echo "TIMING $KIND display ready $(ms) ms"
if [ "$KIND" != x11 ]; then
  # A seat with a keyboard for the whole session: the driver's virtual keyboards come and go
  # with each of its runs, and a seat left without one takes the window's keyboard focus.
  "$root/tools/bin/drive" "$DRIVE" sleep 3600000 >"$OUT/keyboard-holder.log" 2>&1 &
  PIDS+=($!)
  sleep 0.5
fi
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
  --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>' >/dev/null

case "$KIND/$IME" in
  x11/ibus)
    gsettings set org.freedesktop.ibus.engine.hangul initial-input-mode hangul
    gsettings set org.freedesktop.ibus.general preload-engines "['hangul']"
    gsettings set org.freedesktop.ibus.general use-global-engine true
    ibus-daemon --xim --panel=/usr/libexec/ibus-ui-gtk3 -r \
      --address="unix:abstract=skene-ibus-$$" -v >"$OUT/ibus.log" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 100); do ls "$HOME"/.config/ibus/bus/* >/dev/null 2>&1 && break; sleep 0.1; done
    export IBUS_ADDRESS=$(grep -h IBUS_ADDRESS= "$HOME"/.config/ibus/bus/* | cut -d= -f2-)
    for _ in $(seq 100); do
      ibus engine hangul 2>/dev/null && [ "$(ibus engine 2>/dev/null)" = hangul ] && break
      sleep 0.2
    done
    echo "ibus engine: $(ibus engine 2>&1)"
    export XMODIFIERS=@im=ibus GTK_IM_MODULE=ibus
    stdbuf -oL dbus-monitor --address "$IBUS_ADDRESS" >"$OUT/im-bus.log" 2>&1 &
    PIDS+=($!)
    ;;
  x11/fcitx)
    fcitx_profile
    fcitx5 --verbose='*=4' -r >"$OUT/fcitx5.log" 2>&1 &
    PIDS+=($!)
    wait_name org.fcitx.Fcitx5
    sleep 0.5
    export XMODIFIERS=@im=fcitx GTK_IM_MODULE=fcitx
    stdbuf -oL dbus-monitor --session "destination='org.fcitx.Fcitx5'" "sender='org.fcitx.Fcitx5'" \
      >"$OUT/im-bus.log" 2>&1 &
    PIDS+=($!)
    ;;
  sway/fcitx)
    fcitx_profile
    fcitx5 -D --disable=xim,xcb,ibusfrontend,fcitx4frontend,kimpanel,notificationitem \
      --verbose='*=4,waylandim=5' >"$OUT/fcitx5.log" 2>&1 &
    PIDS+=($!)
    wait_name org.fcitx.Fcitx5
    sleep 1
    ;;
  gnome/*)
    for _ in $(seq 100); do
      address=$(ibus address 2>/dev/null)
      [ -n "$address" ] && [ "$address" != "(null)" ] && break
      sleep 0.1
    done
    echo "gnome-shell's ibus at ${address:-none}"
    stdbuf -oL dbus-monitor --address "$address" >"$OUT/im-bus.log" 2>&1 &
    PIDS+=($!)
    ;;
esac
echo "TIMING $KIND input method ready $(ms) ms"

if [ "${APP:-none}" != none ]; then
  # shellcheck disable=SC2086
  env ${APP_ENV:-} "$APP" ${APP_ARGS:-} >"$OUT/app.log" 2>&1 &
  APP_PID=$!
  PIDS+=($APP_PID)
  export APP_PID
  for _ in $(seq 600); do grep -q "^\[frame\]" "$OUT/app.log" 2>/dev/null && break; sleep 0.01; done
  echo "TIMING $KIND first frame $(ms) ms"
  if [ "$KIND" = sway ]; then
    # sway places the window (tiled) after its first frame: the driver aims once it is there.
    for _ in $(seq 300); do grep -q "^\[wayland\] configure .*ACTIVATED" "$OUT/app.log" 2>/dev/null && break; sleep 0.01; done
    sleep 0.2
  fi
fi
"$@"
STATUS=$?
echo "TIMING $KIND session done $(ms) ms"
exit $STATUS
