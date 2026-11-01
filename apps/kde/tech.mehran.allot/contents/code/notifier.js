// Generated from apps/kde/src/notifier.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/**
 * @import {
 *   NotificationButton,
 *   NotificationMessage,
 * } from "./shared/notification-text.js"
 */
var __classPrivateFieldSet = (this && this.__classPrivateFieldSet) || function (receiver, state, value, kind, f) {
    if (kind === "m") throw new TypeError("Private method is not writable");
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
    return (kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value)), value;
};
var __classPrivateFieldGet = (this && this.__classPrivateFieldGet) || function (receiver, state, kind, f) {
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
    return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
};
var _Notifier_send, _Notifier_onButton, _Notifier_onActivate, _Notifier_shown;
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
        _Notifier_send.set(this, void 0);
        _Notifier_onButton.set(this, void 0);
        _Notifier_onActivate.set(this, void 0);
        /** @type {Map<string, SentNotification>} */
        _Notifier_shown.set(this, new Map());
        __classPrivateFieldSet(this, _Notifier_send, options.send, "f");
        __classPrivateFieldSet(this, _Notifier_onButton, options.onButton, "f");
        __classPrivateFieldSet(this, _Notifier_onActivate, options.onActivate, "f");
    }
    /**
     * @param {NotificationMessage} message
     * @param {string} icon
     */
    show(message, icon) {
        this.withdraw(message.tag);
        /** @type {SentNotification} */
        const sent = __classPrivateFieldGet(this, _Notifier_send, "f").call(this, {
            title: message.title,
            body: message.body,
            icon,
            buttons: message.buttons.map((button) => ({
                label: BUTTON_LABELS[button],
                pressed: () => __classPrivateFieldGet(this, _Notifier_onButton, "f").call(this, button, message.target),
            })),
            activated: () => __classPrivateFieldGet(this, _Notifier_onActivate, "f").call(this, message.target),
            // A replaced notification closes after its successor has taken the tag.
            closed: () => {
                if (__classPrivateFieldGet(this, _Notifier_shown, "f").get(message.tag) === sent) {
                    __classPrivateFieldGet(this, _Notifier_shown, "f").delete(message.tag);
                }
            },
        });
        __classPrivateFieldGet(this, _Notifier_shown, "f").set(message.tag, sent);
    }
    /** @param {string} tag */
    withdraw(tag) {
        const sent = __classPrivateFieldGet(this, _Notifier_shown, "f").get(tag);
        if (sent !== undefined) {
            __classPrivateFieldGet(this, _Notifier_shown, "f").delete(tag);
            sent.close();
        }
    }
}
_Notifier_send = new WeakMap(), _Notifier_onButton = new WeakMap(), _Notifier_onActivate = new WeakMap(), _Notifier_shown = new WeakMap();
