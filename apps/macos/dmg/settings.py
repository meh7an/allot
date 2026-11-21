# dmgbuild settings for allot-<version>.dmg: the window design/dmg/DESIGN.md lays out, Allot.app
# on the left and the Applications folder on the right. package-macos.yml passes the built app
# and the background with -D app=<path> -D background=<path>.
import os.path

# dmgbuild runs this file with `defines` already set.
application = defines["app"]  # noqa: F821
appname = os.path.basename(application)

format = "ULFO"
filesystem = "HFS+"
files = [application]
symlinks = {"Applications": "/Applications"}
# Finder labels the app "Allot" whatever its extension setting, as the label pill expects.
hide_extensions = [appname]

background = defines["background"]  # noqa: F821
window_rect = ((200, 240), (640, 400))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
show_icon_preview = False
include_icon_view_settings = True
include_list_view_settings = False

arrange_by = None
icon_size = 128
text_size = 13
icon_locations = {
    appname: (190, 144),
    "Applications": (450, 144),
}
