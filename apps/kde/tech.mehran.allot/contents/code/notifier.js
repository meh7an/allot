// Generated from apps/kde/src/notifier.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/**
 * @import {
 *   NotificationButton,
 *   NotificationMessage,
 * } from "./shared/notification-text.js"
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
    /**
     * @param {object} options
     * @param {(fields: NotificationFields) => SentNotification} options.send
     * @param {(button: NotificationButton, target: string | null) => void} options.onButton
     * @param {(target: string | null) => void} options.onActivate
     */
    constructor(options) {
        /** @type {Map<string, SentNotification>} */
        this._shown = new Map();
        this._send = options.send;
        this._onButton = options.onButton;
        this._onActivate = options.onActivate;
    }
    /**
     * @param {NotificationMessage} message
     * @param {string} icon
     */
    show(message, icon) {
        this.withdraw(message.tag);
        /** @type {SentNotification} */
        const sent = this._send({
            title: message.title,
            body: message.body,
            icon,
            buttons: message.buttons.map((button) => ({
                label: BUTTON_LABELS[button],
                pressed: () => this._onButton(button, message.target),
            })),
            activated: () => this._onActivate(message.target),
            // A replaced notification closes after its successor has taken the tag.
            closed: () => {
                if (this._shown.get(message.tag) === sent) {
                    this._shown.delete(message.tag);
                }
            },
        });
        this._shown.set(message.tag, sent);
    }
    /** @param {string} tag */
    withdraw(tag) {
        const sent = this._shown.get(tag);
        if (sent !== undefined) {
            this._shown.delete(tag);
            sent.close();
        }
    }
}
