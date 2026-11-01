/**
 * @import {
 *   NotificationButton,
 *   NotificationMessage,
 * } from "../../gnome/allot@mehran.tech/lib/notification-text.js"
 */

/** @type {Record<NotificationButton, string>} */
export const BUTTON_LABELS = {
  "start-service": "Start Service",
  "open-logs": "Open Logs",
  show: "Show",
};

/**
 * One notification Plasma holds for the widget.
 *
 * @typedef {object} SentNotification
 * @property {() => void} close
 */

/**
 * What the widget's QML sends.
 *
 * @typedef {object} NotificationFields
 * @property {string} title
 * @property {string} body
 * @property {string} icon
 * @property {{ label: string, pressed: () => void }[]} buttons
 * @property {() => void} activated A click on the notification.
 * @property {() => void} closed A button, a click or the user closed it.
 */

/**
 * Shows notifications through Plasma. A popup that timed out moves to the history without saying
 * so, and changing or resending it there shows nothing. So the same tag always closes the old one
 * and sends a new one, which a merged "3 jobs finished" needs to be seen.
 */
export class Notifier {
  #send;
  #onButton;
  #onActivate;
  /** @type {Map<string, SentNotification>} */
  #shown = new Map();

  /**
   * @param {object} options
   * @param {(fields: NotificationFields) => SentNotification} options.send
   * @param {(button: NotificationButton, target: string | null) => void} options.onButton
   * @param {(target: string | null) => void} options.onActivate
   */
  constructor(options) {
    this.#send = options.send;
    this.#onButton = options.onButton;
    this.#onActivate = options.onActivate;
  }

  /**
   * @param {NotificationMessage} message
   * @param {string} icon
   */
  show(message, icon) {
    this.withdraw(message.tag);
    /** @type {SentNotification} */
    const sent = this.#send({
      title: message.title,
      body: message.body,
      icon,
      buttons: message.buttons.map((button) => ({
        label: BUTTON_LABELS[button],
        pressed: () => this.#onButton(button, message.target),
      })),
      activated: () => this.#onActivate(message.target),
      // A replaced notification closes after its successor has taken the tag.
      closed: () => {
        if (this.#shown.get(message.tag) === sent) {
          this.#shown.delete(message.tag);
        }
      },
    });
    this.#shown.set(message.tag, sent);
  }

  /** @param {string} tag */
  withdraw(tag) {
    const sent = this.#shown.get(tag);
    if (sent !== undefined) {
      this.#shown.delete(tag);
      sent.close();
    }
  }
}
