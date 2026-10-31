import Clutter from "gi://Clutter";
import Gio from "gi://Gio";
import GObject from "gi://GObject";
import St from "gi://St";

import * as PanelMenu from "resource:///org/gnome/shell/ui/panelMenu.js";

import { APP_NAME, accessibleName, iconFile } from "./lib/indicator-status.js";

/** @import { IndicatorStatus } from "./lib/indicator-status.js" */

const ACTIVATING_KEYS = [Clutter.KEY_Return, Clutter.KEY_KP_Enter, Clutter.KEY_space];

/**
 * The panel button. It has no menu of its own: a press opens or closes the Allot popup, which
 * the extension places and dismisses itself so it can stay open.
 */
export const AllotIndicator = GObject.registerClass(
  class AllotIndicator extends PanelMenu.Button {
    /**
     * @param {Gio.File} iconDirectory
     * @param {() => void} onActivate
     */
    _init(iconDirectory, onActivate) {
      super._init(0.5, APP_NAME, true);
      this._iconDirectory = iconDirectory;
      this._onActivate = onActivate;
      this._icon = new St.Icon({ style_class: "system-status-icon" });
      this.add_child(this._icon);
    }

    /** @param {IndicatorStatus} status */
    showStatus(status) {
      this._icon.gicon = new Gio.FileIcon({
        file: this._iconDirectory.get_child(iconFile(status.icon)),
      });
      this.accessible_name = accessibleName(status);
    }

    /** @param {boolean} open */
    setOpen(open) {
      if (open) {
        this.add_style_pseudo_class("checked");
      } else {
        this.remove_style_pseudo_class("checked");
      }
    }

    vfunc_event(event) {
      const type = event.type();
      if (type === Clutter.EventType.BUTTON_PRESS || type === Clutter.EventType.TOUCH_BEGIN) {
        this._onActivate();
        return Clutter.EVENT_STOP;
      }
      return Clutter.EVENT_PROPAGATE;
    }

    vfunc_key_press_event(event) {
      if (ACTIVATING_KEYS.includes(event.get_key_symbol())) {
        this._onActivate();
        return Clutter.EVENT_STOP;
      }
      return super.vfunc_key_press_event(event);
    }
  },
);
