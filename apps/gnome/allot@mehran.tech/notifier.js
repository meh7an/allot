import Gio from "gi://Gio";

import * as Main from "resource:///org/gnome/shell/ui/main.js";
import * as MessageTray from "resource:///org/gnome/shell/ui/messageTray.js";

import { APP_NAME } from "./lib/indicator-status.js";

/** @import { NotificationMessage, NotificationButton } from "./lib/notification-text.js" */

const BUTTON_LABELS = {
  "start-service": "Start Service",
  "open-logs": "Open Logs",
  show: "Show",
};

/**
 * Shows notifications in the shell's message tray. One source holds them, made when the first one
 * shows, since a source destroys itself with its last notification.
 */
export class Notifier {
  #onButton;
  #onActivate;
  #currentIcon;
  /** @type {MessageTray.Source | null} */
  #source = null;
  /** @type {Map<string, MessageTray.Notification>} */
  #shown = new Map();

  /**
   * @param {object} options
   * @param {(button: NotificationButton, target: string | null) => void} options.onButton
   * @param {(target: string | null) => void} options.onActivate A click on the notification.
   * @param {() => Gio.Icon} options.currentIcon The panel icon of this moment, as the logo.
   */
  constructor(options) {
    this.#onButton = options.onButton;
    this.#onActivate = options.onActivate;
    this.#currentIcon = options.currentIcon;
  }

  /** @param {NotificationMessage} message */
  show(message) {
    const existing = this.#shown.get(message.tag);
    if (existing !== undefined) {
      // In place, so a merged "3 jobs finished" replaces its banner without closing the source.
      existing.set({ title: message.title, body: message.body, gicon: this.#currentIcon() });
      existing.acknowledged = false;
      console.log(`Allot: notified ${message.tag} again`);
      return;
    }
    const source = this.#ensureSource();
    const notification = new MessageTray.Notification({
      source,
      title: message.title,
      body: message.body,
      gicon: this.#currentIcon(),
    });
    for (const button of message.buttons) {
      notification.addAction(BUTTON_LABELS[button], () =>
        this.#onButton(button, message.target),
      );
    }
    notification.connect("activated", () => this.#onActivate(message.target));
    notification.connect("destroy", () => {
      if (this.#shown.get(message.tag) === notification) {
        this.#shown.delete(message.tag);
      }
    });
    this.#shown.set(message.tag, notification);
    source.addNotification(notification);
    console.log(`Allot: notified ${message.tag}`);
  }

  /** @param {string} tag */
  withdraw(tag) {
    this.#shown.get(tag)?.destroy(MessageTray.NotificationDestroyedReason.SOURCE_CLOSED);
  }

  destroy() {
    this.#source?.destroy();
    this.#source = null;
    this.#shown.clear();
  }

  /** @returns {MessageTray.Source} */
  #ensureSource() {
    if (this.#source === null) {
      const source = new MessageTray.Source({ title: APP_NAME, icon: this.#currentIcon() });
      source.connect("destroy", () => {
        if (this.#source === source) {
          this.#source = null;
        }
      });
      Main.messageTray.add(source);
      this.#source = source;
    }
    return this.#source;
  }
}
