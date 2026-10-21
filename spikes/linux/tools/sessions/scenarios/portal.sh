# The settings portal read at start (the stand-in started with PORTAL: dark, PORTAL_READ_ALL
# saying how it answers `ReadAll`, its calls in portal.log): a portal without `ReadAll` is read a
# key at a time, and one that does not answer is not asked again, the window drawn light within
# the connection's timeout. MODE: unknown or silent.
. "$(dirname "$0")/common.sh"
calls() { grep -c "^$1" "$OUT/portal.log"; }
case $MODE in
  unknown)
    await_shot portal-$KIND-dark 200 300 1c1c1e
    pixel portal-$KIND-dark 200 300 1c1c1e
    expect "[portal] org.freedesktop.appearance color-scheme = U32(1)" "a portal without ReadAll is read a key at a time"
    if [ "$(calls ReadAll)" = 1 ] && [ "$(calls ReadOne)" -ge 4 ]; then
      pass "$KIND: ReadAll asked once, then ReadOne per key ($(calls ReadOne))"
    else
      fail "$KIND: ReadAll asked once, then ReadOne per key (ReadAll $(calls ReadAll), ReadOne $(calls ReadOne))"
    fi
    ;;
  silent)
    await_shot portal-$KIND-light 200 300 ffffff
    pixel portal-$KIND-light 200 300 ffffff
    absent "^\[portal\] org" "a portal that does not answer gives no setting"
    if [ "$(calls ReadAll)" = 1 ] && [ "$(calls ReadOne)" = 0 ] && [ "$(calls Read\ )" = 0 ]; then
      pass "$KIND: a portal that does not answer ReadAll is asked nothing more"
    else
      fail "$KIND: a portal that does not answer ReadAll is asked nothing more (ReadAll $(calls ReadAll), ReadOne $(calls ReadOne), Read $(calls Read\ ))"
    fi
    ;;
esac
