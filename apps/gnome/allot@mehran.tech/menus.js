import Clutter from "gi://Clutter";
import GObject from "gi://GObject";
import St from "gi://St";

import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";

import { meterCount, notificationCount } from "./lib/menu-counts.js";
import { DO_NOT_DISTURB_TEXT } from "./lib/snapshot.js";

/** @import { PopupModel } from "./lib/popup-model.js" */
/** @import { TraySettings } from "./lib/settings.js" */

/**
 * A check item that stays open when pressed, as the handoff asks: an item that emitted `activate`
 * would close the whole menu.
 */
const CheckItem = GObject.registerClass(
  class AllotCheckItem extends PopupMenu.PopupBaseMenuItem {
    /**
     * @param {string} text
     * @param {() => void} onToggle
     */
    _init(text, onToggle) {
      super._init();
      this._onToggle = onToggle;
      this.label = new St.Label({ text, y_align: Clutter.ActorAlign.CENTER });
      this.add_child(this.label);
      this.label_actor = this.label;
    }

    /** @param {boolean} checked */
    setChecked(checked) {
      this.setOrnament(checked ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
    }

    activate(_event) {
      this._onToggle();
    }
  },
);

/**
 * A submenu with a count before its arrow.
 *
 * @param {PopupMenu.PopupMenu} menu
 * @param {string} title
 * @returns {{ item: PopupMenu.PopupSubMenuMenuItem, count: St.Label }}
 */
function countedSubmenu(menu, title) {
  const item = new PopupMenu.PopupSubMenuMenuItem(title);
  const count = new St.Label({
    style_class: "allot-menu-count",
    y_align: Clutter.ActorAlign.CENTER,
  });
  // The last child is the arrow.
  item.insert_child_at_index(count, item.get_n_children() - 1);
  menu.addMenuItem(item);
  return { item, count };
}

/**
 * The header's More options menu: Meters and Notifications, whose items stay open while toggled,
 * and Open Logs. Its items are updated in place, so a poll never disturbs an open submenu.
 */
export class MoreOptionsMenu {
  menu;
  #model;
  #onChanged;
  #meterCount;
  #notificationCount;
  /** @type {Array<{ item: any, read: (settings: TraySettings) => boolean }>} */
  #checks = [];
  #vram;
  #driveActivity;
  #doNotDisturb;

  /**
   * @param {object} options
   * @param {St.Widget} options.source The button the menu hangs from.
   * @param {PopupModel} options.model
   * @param {() => void} options.onOpenLogs
   * @param {() => void} options.onChanged A toggle changed the model.
   */
  constructor(options) {
    this.#model = options.model;
    this.#onChanged = options.onChanged;
    this.menu = new PopupMenu.PopupMenu(options.source, 0.5, St.Side.TOP);

    const meters = countedSubmenu(this.menu, "Meters");
    this.#meterCount = meters.count;
    const meterCheck = (
      /** @type {string} */ text,
      /** @type {keyof TraySettings["meters"]} */ key,
    ) =>
      this.#addCheck(
        meters.item.menu,
        text,
        (settings) => settings.meters[key],
        (settings) => {
          settings.meters[key] = !settings.meters[key];
        },
      );
    meterCheck("CPU", "cpu");
    meterCheck("RAM", "ram");
    meterCheck("GPU", "gpu");
    this.#vram = meterCheck("VRAM", "vram");
    meterCheck("Drives", "drives");
    this.#driveActivity = meterCheck("Drive Activity", "drive_activity");
    this.#driveActivity.add_style_class_name("allot-menu-indented");

    const notifications = countedSubmenu(this.menu, "Notifications");
    this.#notificationCount = notifications.count;
    this.#doNotDisturb = new PopupMenu.PopupMenuItem(DO_NOT_DISTURB_TEXT, { reactive: false });
    notifications.item.menu.addMenuItem(this.#doNotDisturb);
    const notificationCheck = (
      /** @type {string} */ text,
      /** @type {keyof TraySettings["notifications"]} */ key,
    ) =>
      this.#addCheck(
        notifications.item.menu,
        text,
        (settings) => settings.notifications[key],
        (settings) => {
          settings.notifications[key] = !settings.notifications[key];
        },
      );
    notificationCheck("Service Stopped", "service_stopped");
    notificationCheck("Running Long", "running_long");
    notificationCheck("Benchmark Finished", "benchmark_finished");
    notificationCheck("Job Finished", "job_finished");
    notificationCheck("Killed Outside the Popup", "killed_outside_flyout");

    this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
    this.menu.addAction("Open Logs", options.onOpenLogs);
    this.sync();
  }

  /** Shows the model's settings, in place. */
  sync() {
    const { settings, resources, doNotDisturb } = this.#model;
    for (const { item, read } of this.#checks) {
      item.setChecked(read(settings));
    }
    this.#meterCount.text = meterCount(settings.meters);
    this.#notificationCount.text = notificationCount(settings.notifications);
    this.#vram.label.text = resources.sharesMemory ? "GPU Memory" : "VRAM";
    this.#driveActivity.setSensitive(settings.meters.drives);
    this.#doNotDisturb.visible = doNotDisturb;
  }

  /**
   * @param {PopupMenu.PopupSubMenu} submenu
   * @param {string} text
   * @param {(settings: TraySettings) => boolean} read
   * @param {(settings: TraySettings) => void} toggle
   * @returns {any} The item.
   */
  #addCheck(submenu, text, read, toggle) {
    const item = new CheckItem(text, () => {
      this.#model.updateSettings(toggle);
      this.sync();
      this.#onChanged();
    });
    submenu.addMenuItem(item);
    this.#checks.push({ item, read });
    return item;
  }
}

/**
 * The panel button's right-click menu: the summary, Stop Service or Start Service, and Open Logs.
 * There is no Launch at Login, because the shell starts the extension, and no Quit, because an
 * extension is turned off from GNOME's Extensions app.
 */
export class PanelContextMenu {
  menu;
  #summary;
  #service;
  /** @type {() => void} */
  #onService;

  /**
   * @param {object} options
   * @param {St.Widget} options.source The panel button.
   * @param {() => void} options.onService The service item was pressed.
   * @param {() => void} options.onOpenLogs
   */
  constructor(options) {
    this.#onService = options.onService;
    this.menu = new PopupMenu.PopupMenu(options.source, 0.5, St.Side.TOP);
    this.#summary = new PopupMenu.PopupMenuItem("", { reactive: false });
    this.menu.addMenuItem(this.#summary);
    this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
    this.#service = this.menu.addAction("", () => this.#onService());
    this.menu.addAction("Open Logs", options.onOpenLogs);
  }

  /**
   * @param {string} summary
   * @param {import("./lib/service-menu.js").ServiceMenuItem} service
   */
  sync(summary, service) {
    this.#summary.label.text = summary;
    this.#service.label.text = service.title;
    this.#service.setSensitive(service.action !== "none");
  }
}
