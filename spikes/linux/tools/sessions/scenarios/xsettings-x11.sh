# XSETTINGS on X11 (an XSETTINGS manager started first, BEFORE_PID): Xft/DPI 144 scales by 1.5,
# Net/ThemeName Adwaita-dark is the dark scheme, Gtk/EnableAnimations 0 is reduced motion; at the
# manager's next step, the scenario's, DPI 96 and HighContrast: scale 1 at the same window size,
# more contrast.
. "$(dirname "$0")/common.sh"
await_shot xsettings-150 300 450 1c1c1e
size xsettings-150 720 960
pixel xsettings-150 300 450 1c1c1e
expect "[xsettings] 3 settings, dpi Some(147456), scale 1.5" "Xft/DPI 144 scales by 1.5"
step "$BEFORE_PID"; await_shot xsettings-100 300 450 ffff00
size xsettings-100 720 960
pixel xsettings-100 300 450 ffff00
expect "[xsettings] 3 settings, dpi Some(98304), scale 1" "a new Xft/DPI is followed"
