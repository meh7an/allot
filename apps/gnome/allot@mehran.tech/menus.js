import St from "gi://St";

import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";

import { SETTINGS_TITLE, serviceMenuItem } from "./lib/service-menu.js";

/** @import { PopupModel } from "./lib/popup-model.js" */

/**
 * The header's More options menu (DESIGN.md section 9): Settings, Stop Service or Start Service,
 * and Open Logs. Meters and Notifications moved to the Settings window's Panel page.
 */
export class MoreOptionsMenu {
  menu;
  #model;
  #service;

  /**
   * @param {object} options
   * @param {St.Widget} options.source The button the menu hangs from.
   * @param {PopupModel} options.model
   * @param {() => void} options.onSettings
   * @param {() => void} options.onService The service item was pressed.
   * @param {() => void} options.onOpenLogs
   */
  constructor(options) {
    this.#model = options.model;
    this.menu = new PopupMenu.PopupMenu(options.source, 0.5, St.Side.TOP);
    this.menu.addAction(SETTINGS_TITLE, options.onSettings);
    this.#service = this.menu.addAction("", options.onService);
    this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
    this.menu.addAction("Open Logs", options.onOpenLogs);
    this.sync();
  }

  /** Shows the service item for the model's status, in place. */
  sync() {
    const service = serviceMenuItem(this.#model.status);
    this.#service.label.text = service.title;
    this.#service.setSensitive(service.action !== "none");
  }
}

/**
 * The panel button's right-click menu: the summary, Settings, Stop Service or Start Service, and
 * Open Logs.
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
   * @param {() => void} options.onSettings
   * @param {() => void} options.onService The service item was pressed.
   * @param {() => void} options.onOpenLogs
   */
  constructor(options) {
    this.#onService = options.onService;
    this.menu = new PopupMenu.PopupMenu(options.source, 0.5, St.Side.TOP);
    this.#summary = new PopupMenu.PopupMenuItem("", { reactive: false });
    this.menu.addMenuItem(this.#summary);
    this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
    this.menu.addAction(SETTINGS_TITLE, options.onSettings);
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
