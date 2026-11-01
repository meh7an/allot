// Generated from apps/kde/src/widget.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
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
var _AllotWidget_instances, _AllotWidget_host, _AllotWidget_runner, _AllotWidget_transport, _AllotWidget_keeper, _AllotWidget_appIcons, _AllotWidget_rules, _AllotWidget_notifier, _AllotWidget_configFolder, _AllotWidget_dataFolder, _AllotWidget_poller, _AllotWidget_lastStatus, _AllotWidget_tick, _AllotWidget_saving, _AllotWidget_expanded, _AllotWidget_stopped, _AllotWidget_show, _AllotWidget_notify, _AllotWidget_pressNotificationButton, _AllotWidget_showRow, _AllotWidget_stopService, _AllotWidget_saveSettings, _AllotWidget_showPress, _AllotWidget_showChange, _AllotWidget_render, _AllotWidget_scheduleTick, _AllotWidget_clearTick;
import { DaemonClient } from "./shared/client.js";
import { DEFAULT_PORT, dataPaths, portFromToml } from "./shared/config.js";
import { accessibleName, iconFile } from "./shared/indicator-status.js";
import { meterCount, notificationCount } from "./shared/menu-counts.js";
import { NotificationRules } from "./shared/notification-rules.js";
import { DaemonPoller } from "./shared/poller.js";
import { PopupModel } from "./shared/popup-model.js";
import { SystemdServiceLauncher } from "./shared/service-launcher.js";
import { STOP_CONFIRM_TITLE, serviceMenuItem, stopConfirmation, } from "./shared/service-menu.js";
import { decodeSettings, encodeSettings } from "./shared/settings.js";
import { DO_NOT_DISTURB_TEXT, WELCOME_BODY, WELCOME_TITLE, } from "./shared/snapshot.js";
import { AppIcons } from "./app-icons.js";
import { CommandRunner, XhrTransport, copyFile, detachedCommand, localPath, readTextFile, serviceSystem, writeTextFile, } from "./io.js";
import { INHIBITED_COMMAND, NOTIFY_COMPONENT, NOTIFY_EVENT, NOTIFY_RC_FILE, SETTINGS_ARGV, isOffInSettings, parseInhibited, } from "./notification-state.js";
import { Notifier } from "./notifier.js";
import { RowKeeper, listRows } from "./rows.js";
/** @import { PollResult, ServiceStatus } from "./shared/client.js" */
/** @import { DataPaths } from "./shared/config.js" */
/** @import { Timers } from "./shared/poller.js" */
/** @import { NotificationButton } from "./shared/notification-text.js" */
/** @import { ModelChange } from "./shared/popup-model.js" */
/** @import { ServiceMenuItem } from "./shared/service-menu.js" */
/** @import { MeterToggles } from "./shared/settings.js" */
/** @import { NotificationToggles } from "./shared/settings.js" */
/** @import { IdleLine } from "./shared/snapshot.js" */
/** @import { JobGroupKind } from "./shared/snapshot.js" */
/** @import { XhrFactory } from "./io.js" */
/** @import { NotificationFields, SentNotification } from "./notifier.js" */
/** @import { Row, RowModel } from "./rows.js" */
export { DO_NOT_DISTURB_TEXT, NOTIFY_COMPONENT, NOTIFY_EVENT, STOP_CONFIRM_TITLE, WELCOME_BODY, WELCOME_TITLE, };
/** Breeze icons for the group headings, the handoff's timer, play and clock. */
export const GROUP_ICONS = {
    benchmark: "chronometer",
    running: "media-playback-start",
    waiting: "clock",
};
export const NOTHING_TEXT = "No jobs running or waiting.";
/** The opacity of the idle header glyph and of the last known jobs while allotd is away. */
export const DIMMED = 0.5;
/** The header glyph while the service is not serving state. */
export const ERROR_ICON = "dialog-error";
/** The header glyph otherwise, under the package's `icons` folder; dimmed while idle. */
export const MARK_ICON_FILE = "allot-mark-symbolic.svg";
/** The Notifications submenu's first line while System Settings has them off. */
export const NOTIFICATIONS_OFF_TEXT = "Turned off in System Settings";
/** The first-run card's line for the same. */
export const NOTIFICATIONS_OFF_CARD_TEXT = "Notifications are turned off in System Settings.";
/** The menu's count while System Settings has them off. */
export const NOTIFICATIONS_OFF_COUNT = "Off";
// Times on the cards tick once a second while the popup is open.
const TICK_MS = 1000;
/**
 * What the widget needs from its QML item.
 *
 * @typedef {object} WidgetHost
 * @property {string} homeUrl `StandardPaths`' home location.
 * @property {string} configUrl `StandardPaths`' config location.
 * @property {string} dataUrl `StandardPaths`' generic data location.
 * @property {string} notifyRcUrl The notifyrc in the package.
 * @property {(command: string) => void} connectSource The executable data source's
 *   `connectSource`; its `onNewData` calls `deliver`.
 * @property {XhrFactory} requests
 * @property {Timers} timers
 * @property {RowModel} rows The ListModel the popup's list shows.
 * @property {(data: Record<string, Row>, animate: boolean) => void} showRows
 *   Hands the views their rows, by key; `animate` is off when the popup opens.
 * @property {(change: ModelChange) => void} onChange Called after the model changed.
 * @property {(fields: NotificationFields) => SentNotification} sendNotification
 * @property {() => void} expand Opens the popup.
 * @property {(key: string | null) => void} scrollTo Scrolls the open popup to a row, or the top.
 * @property {(url: string) => void} openUrl
 * @property {(line: string) => void} log
 * @property {(ms: number) => string} [timeOfDay] A daemon time as the user's short time of day.
 * @property {() => number} [now] The clock that ticks times and stops; the tests pass their own.
 */
/**
 * The widget's state and polling. Plasma's own Keep Open pin is the popup's Stay open: the popup
 * then stays up when another window takes focus. Plasma anchors the popup to the panel, so
 * there is no Pin with dragging.
 */
export class AllotWidget {
    /** @param {WidgetHost} host */
    constructor(host) {
        _AllotWidget_instances.add(this);
        /**
         * The question the popup's Stop card asks, or empty.
         *
         * @type {string}
         */
        this.stopQuestion = "";
        /**
         * Allot's notifications are turned off in System Settings.
         *
         * @type {boolean}
         */
        this.notificationsOff = false;
        _AllotWidget_host.set(this, void 0);
        _AllotWidget_runner.set(this, void 0);
        _AllotWidget_transport.set(this, void 0);
        _AllotWidget_keeper.set(this, void 0);
        _AllotWidget_appIcons.set(this, void 0);
        _AllotWidget_rules.set(this, void 0);
        _AllotWidget_notifier.set(this, void 0);
        _AllotWidget_configFolder.set(this, void 0);
        _AllotWidget_dataFolder.set(this, void 0);
        /** @type {DaemonPoller | null} */
        _AllotWidget_poller.set(this, null);
        /** @type {ServiceStatus | null} */
        _AllotWidget_lastStatus.set(this, null);
        /** @type {unknown} */
        _AllotWidget_tick.set(this, null);
        /** @type {Promise<void>} */
        _AllotWidget_saving.set(this, Promise.resolve());
        _AllotWidget_expanded.set(this, false);
        _AllotWidget_stopped.set(this, false);
        __classPrivateFieldSet(this, _AllotWidget_host, host, "f");
        this.model = new PopupModel(host.now);
        this.model.onChange = (change) => __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, change);
        this.model.onStopAnswered = () => { var _a; return (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.pollNow(); };
        this.model.onKillSent = (lease) => __classPrivateFieldGet(this, _AllotWidget_rules, "f").noteLocalKill(lease);
        this.model.onSettingsChanged = () => __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_saveSettings).call(this);
        if (host.timeOfDay !== undefined) {
            this.model.timeOfDay = host.timeOfDay;
        }
        this.paths = dataPaths(localPath(host.homeUrl));
        __classPrivateFieldSet(this, _AllotWidget_configFolder, localPath(host.configUrl), "f");
        __classPrivateFieldSet(this, _AllotWidget_dataFolder, localPath(host.dataUrl), "f");
        __classPrivateFieldSet(this, _AllotWidget_runner, new CommandRunner(host.connectSource), "f");
        __classPrivateFieldSet(this, _AllotWidget_transport, new XhrTransport(host.requests, host.timers), "f");
        __classPrivateFieldSet(this, _AllotWidget_keeper, new RowKeeper(host.rows), "f");
        __classPrivateFieldSet(this, _AllotWidget_appIcons, new AppIcons(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), () => {
            __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, { snapshot: true, indicator: false });
        }), "f");
        __classPrivateFieldSet(this, _AllotWidget_rules, new NotificationRules(host.now), "f");
        __classPrivateFieldSet(this, _AllotWidget_notifier, new Notifier({
            send: host.sendNotification,
            onButton: (button, target) => __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_pressNotificationButton).call(this, button, target),
            onActivate: (target) => __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showRow).call(this, target),
        }), "f");
    }
    /**
     * Reads the settings and the port once, as the other apps do at launch, puts the notifyrc where
     * Plasma looks for it, then polls until `stop`. A widget's package cannot install that file.
     *
     * @returns {Promise<void>}
     */
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            const [toml, settings, notifyRc] = yield Promise.all([
                readTextFile(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), this.paths.config),
                readTextFile(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), this.paths.settings),
                copyFile(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), localPath(__classPrivateFieldGet(this, _AllotWidget_host, "f").notifyRcUrl), `${__classPrivateFieldGet(this, _AllotWidget_dataFolder, "f")}/knotifications6/${NOTIFY_RC_FILE}`),
            ]);
            if (__classPrivateFieldGet(this, _AllotWidget_stopped, "f")) {
                return;
            }
            if (!notifyRc) {
                __classPrivateFieldGet(this, _AllotWidget_host, "f").log("Allot: cannot install the notifyrc; notifications will not show");
            }
            __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, this.model.loadSettings(decodeSettings(settings)));
            this.refreshNotificationState();
            const client = new DaemonClient({
                port: toml === null ? DEFAULT_PORT : portFromToml(toml),
                send: __classPrivateFieldGet(this, _AllotWidget_transport, "f").send,
                readToken: () => readTextFile(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), this.paths.token),
            });
            this.model.stopper = client;
            const launcher = new SystemdServiceLauncher({
                home: localPath(__classPrivateFieldGet(this, _AllotWidget_host, "f").homeUrl),
                configHome: __classPrivateFieldGet(this, _AllotWidget_configFolder, "f"),
                root: this.paths.root,
            }, serviceSystem(__classPrivateFieldGet(this, _AllotWidget_runner, "f")));
            const poller = new DaemonPoller(client, __classPrivateFieldGet(this, _AllotWidget_host, "f").timers, Object.assign({ launcher }, (__classPrivateFieldGet(this, _AllotWidget_host, "f").now === undefined ? {} : { now: __classPrivateFieldGet(this, _AllotWidget_host, "f").now })));
            poller.onUpdate = (result) => __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_show).call(this, result);
            poller.setPopupOpen(__classPrivateFieldGet(this, _AllotWidget_expanded, "f"));
            __classPrivateFieldSet(this, _AllotWidget_poller, poller, "f");
            try {
                yield poller.start();
            }
            catch (error) {
                __classPrivateFieldGet(this, _AllotWidget_host, "f").log(`Allot: polling stopped: ${error}`);
            }
        });
    }
    stop() {
        var _a;
        __classPrivateFieldSet(this, _AllotWidget_stopped, true, "f");
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_clearTick).call(this);
        (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.stop();
        __classPrivateFieldGet(this, _AllotWidget_transport, "f").cancel();
    }
    /**
     * The executable data source's answers.
     *
     * @param {string} command
     * @param {Record<string, unknown>} data
     */
    deliver(command, data) {
        __classPrivateFieldGet(this, _AllotWidget_runner, "f").deliver(command, data);
    }
    /**
     * Follows Plasma's `expanded`: opening polls at once, starts without Keep Open and shows the
     * rows without animating them; closing takes back any Kill? and the Stop card.
     *
     * @param {boolean} expanded
     */
    setExpanded(expanded) {
        var _a;
        __classPrivateFieldSet(this, _AllotWidget_expanded, expanded, "f");
        if (expanded) {
            this.model.resetForOpen();
            this.model.refresh();
            __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_render).call(this, false);
            this.refreshNotificationState();
        }
        else {
            this.model.cancelConfirms();
            __classPrivateFieldGet(this, _AllotWidget_appIcons, "f").forget();
            this.stopQuestion = "";
        }
        (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.setPopupOpen(expanded);
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_scheduleTick).call(this);
    }
    toggleKeepOpen() {
        this.model.toggleStayOpen();
    }
    /**
     * A press of Kill, or of Kill? within its countdown.
     *
     * @param {number} lease
     */
    pressKill(lease) {
        this.model.pressKill(lease);
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showPress).call(this);
    }
    /**
     * A press of Remove on a queued request.
     *
     * @param {number} lease
     */
    pressRemove(lease) {
        this.model.pressRemove(lease);
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showPress).call(this);
    }
    /**
     * Focus left a Kill? confirm.
     *
     * @param {number} lease
     */
    cancelConfirm(lease) {
        if (this.model.cancelConfirm(lease)) {
            __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showPress).call(this);
        }
    }
    /** Esc takes back a Kill? confirm before it closes the popup. */
    cancelConfirms() {
        if (this.model.cancelConfirms()) {
            __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showPress).call(this);
        }
    }
    /**
     * A leaving row's view has collapsed.
     *
     * @param {string} key
     */
    rowGone(key) {
        __classPrivateFieldGet(this, _AllotWidget_keeper, "f").gone(key);
    }
    /** The Resources card's header. */
    toggleResources() {
        this.model.toggleResources();
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showPress).call(this);
    }
    /**
     * A Meters check item.
     *
     * @param {keyof MeterToggles} key
     */
    toggleMeter(key) {
        this.model.updateSettings((settings) => {
            settings.meters[key] = !settings.meters[key];
        });
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showPress).call(this);
    }
    /**
     * A Notifications check item.
     *
     * @param {keyof NotificationToggles} key
     */
    toggleNotification(key) {
        this.model.updateSettings((settings) => {
            settings.notifications[key] = !settings.notifications[key];
        });
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showPress).call(this);
    }
    /** The first-run card's Done. */
    finishFirstRun() {
        this.model.finishFirstRun();
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showPress).call(this);
    }
    /** The banner's Start Service, or the panel menu's while allotd is down. */
    startService() {
        return __awaiter(this, void 0, void 0, function* () {
            const poller = __classPrivateFieldGet(this, _AllotWidget_poller, "f");
            if (poller === null) {
                return;
            }
            __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, this.model.noteStarting());
            const problem = yield poller.startService();
            if (problem !== null) {
                __classPrivateFieldGet(this, _AllotWidget_host, "f").log(`Allot: cannot start the service: ${problem}`);
            }
        });
    }
    /** The panel menu's service item: with jobs up, Stop asks in the popup first. */
    pressServiceItem() {
        var _a;
        var _b;
        switch (serviceMenuItem(this.model.status).action) {
            case "start":
                this.startService();
                break;
            case "stop": {
                const question = stopConfirmation((_b = (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.lastKnown) !== null && _b !== void 0 ? _b : null);
                if (question === null) {
                    __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_stopService).call(this);
                }
                else {
                    this.stopQuestion = question;
                    __classPrivateFieldGet(this, _AllotWidget_host, "f").expand();
                    __classPrivateFieldGet(this, _AllotWidget_host, "f").onChange({ snapshot: false, indicator: false });
                }
                break;
            }
            case "none":
                break;
        }
    }
    /** The Stop card's Stop. */
    confirmStop() {
        this.cancelStop();
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_stopService).call(this);
    }
    /** The Stop card's Cancel. */
    cancelStop() {
        this.stopQuestion = "";
        __classPrivateFieldGet(this, _AllotWidget_host, "f").onChange({ snapshot: false, indicator: false });
    }
    /**
     * Reads whether Plasma would show notifications now: when the popup or More options opens, since
     * Plasma tells the widget of neither switch.
     */
    refreshNotificationState() {
        return __awaiter(this, void 0, void 0, function* () {
            const [inhibited, events, plasma] = yield Promise.all([
                __classPrivateFieldGet(this, _AllotWidget_runner, "f").run(INHIBITED_COMMAND),
                readTextFile(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), `${__classPrivateFieldGet(this, _AllotWidget_configFolder, "f")}/${NOTIFY_RC_FILE}`),
                readTextFile(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), `${__classPrivateFieldGet(this, _AllotWidget_configFolder, "f")}/plasmanotifyrc`),
            ]);
            if (__classPrivateFieldGet(this, _AllotWidget_stopped, "f")) {
                return;
            }
            const off = isOffInSettings(events, plasma);
            const doNotDisturb = inhibited.exitCode === 0 && parseInhibited(inhibited.stdout);
            if (off !== this.notificationsOff || doNotDisturb !== this.model.doNotDisturb) {
                this.notificationsOff = off;
                this.model.setDoNotDisturb(doNotDisturb);
                __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showPress).call(this);
            }
        });
    }
    /** The menu's and the first-run card's way to turn notifications back on. */
    openNotificationSettings() {
        __classPrivateFieldGet(this, _AllotWidget_runner, "f").run(detachedCommand(SETTINGS_ARGV));
    }
    /** Opens the logs folder. */
    openLogs() {
        __classPrivateFieldGet(this, _AllotWidget_host, "f").openUrl(`file://${this.paths.logs}`);
    }
    /** @returns {boolean} */
    get keepOpen() {
        return this.model.isStayOpen;
    }
    /** @returns {string} The panel icon's file under the package's `icons` folder. */
    get iconFile() {
        return iconFile(this.model.indicator.icon);
    }
    /** @returns {string} */
    get accessibleName() {
        return accessibleName(this.model.indicator);
    }
    /** @returns {ServiceMenuItem} */
    get serviceItem() {
        return serviceMenuItem(this.model.status);
    }
    /** @returns {string} */
    get meterCount() {
        return meterCount(this.model.settings.meters);
    }
    /** @returns {string} */
    get notificationCount() {
        return this.notificationsOff
            ? NOTIFICATIONS_OFF_COUNT
            : notificationCount(this.model.settings.notifications);
    }
}
_AllotWidget_host = new WeakMap(), _AllotWidget_runner = new WeakMap(), _AllotWidget_transport = new WeakMap(), _AllotWidget_keeper = new WeakMap(), _AllotWidget_appIcons = new WeakMap(), _AllotWidget_rules = new WeakMap(), _AllotWidget_notifier = new WeakMap(), _AllotWidget_configFolder = new WeakMap(), _AllotWidget_dataFolder = new WeakMap(), _AllotWidget_poller = new WeakMap(), _AllotWidget_lastStatus = new WeakMap(), _AllotWidget_tick = new WeakMap(), _AllotWidget_saving = new WeakMap(), _AllotWidget_expanded = new WeakMap(), _AllotWidget_stopped = new WeakMap(), _AllotWidget_instances = new WeakSet(), _AllotWidget_show = function _AllotWidget_show(result) {
    var _a, _b;
    var _c, _d, _e, _f;
    if (result.status !== __classPrivateFieldGet(this, _AllotWidget_lastStatus, "f")) {
        __classPrivateFieldSet(this, _AllotWidget_lastStatus, result.status, "f");
        const detail = (_d = (_c = result.version) !== null && _c !== void 0 ? _c : result.detail) !== null && _d !== void 0 ? _d : "";
        __classPrivateFieldGet(this, _AllotWidget_host, "f").log(`Allot: service ${result.status}${detail === "" ? "" : `: ${detail}`}`);
    }
    const lastKnown = (_e = (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.lastKnown) !== null && _e !== void 0 ? _e : null;
    const startProblem = (_f = (_b = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _b === void 0 ? void 0 : _b.startProblem) !== null && _f !== void 0 ? _f : null;
    __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, this.model.apply(result, lastKnown, startProblem));
    __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_notify).call(this, result);
    if (this.model.wantsFirstOpen) {
        this.model.noteFirstOpened();
        __classPrivateFieldGet(this, _AllotWidget_host, "f").expand();
    }
}, _AllotWidget_notify = function _AllotWidget_notify(result) {
    var _a;
    var _b;
    const update = __classPrivateFieldGet(this, _AllotWidget_rules, "f").observe({
        status: result.status,
        live: result.state,
        lastKnown: (_b = (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.lastKnown) !== null && _b !== void 0 ? _b : null,
        toggles: this.model.settings.notifications,
        popupOpen: __classPrivateFieldGet(this, _AllotWidget_expanded, "f"),
    });
    for (const message of update.show) {
        __classPrivateFieldGet(this, _AllotWidget_notifier, "f").show(message, this.iconFile);
        __classPrivateFieldGet(this, _AllotWidget_host, "f").log(`Allot: notified ${message.tag}`);
    }
    for (const tag of update.withdraw) {
        __classPrivateFieldGet(this, _AllotWidget_notifier, "f").withdraw(tag);
    }
}, _AllotWidget_pressNotificationButton = function _AllotWidget_pressNotificationButton(button, target) {
    switch (button) {
        case "start-service":
            this.startService();
            break;
        case "open-logs":
            this.openLogs();
            break;
        case "show":
            __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showRow).call(this, target);
            break;
    }
}, _AllotWidget_showRow = function _AllotWidget_showRow(target) {
    __classPrivateFieldGet(this, _AllotWidget_host, "f").expand();
    __classPrivateFieldGet(this, _AllotWidget_host, "f").scrollTo(target);
}, _AllotWidget_stopService = function _AllotWidget_stopService() {
    return __awaiter(this, void 0, void 0, function* () {
        const poller = __classPrivateFieldGet(this, _AllotWidget_poller, "f");
        if (poller === null) {
            return;
        }
        __classPrivateFieldGet(this, _AllotWidget_rules, "f").expectStop();
        try {
            const problem = yield poller.stopService();
            if (problem !== null) {
                __classPrivateFieldGet(this, _AllotWidget_host, "f").log(`Allot: cannot stop the service: ${problem}`);
            }
        }
        catch (error) {
            // Only the widget going away cancels the request.
            __classPrivateFieldGet(this, _AllotWidget_host, "f").log(`Allot: stop not finished: ${error}`);
        }
    });
}, _AllotWidget_saveSettings = function _AllotWidget_saveSettings() {
    const text = encodeSettings(this.model.settings);
    __classPrivateFieldSet(this, _AllotWidget_saving, __classPrivateFieldGet(this, _AllotWidget_saving, "f").then(() => __awaiter(this, void 0, void 0, function* () {
        if (!(yield writeTextFile(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), this.paths.settings, text))) {
            __classPrivateFieldGet(this, _AllotWidget_host, "f").log(`Allot: cannot save ${this.paths.settings}`);
        }
    })), "f");
}, _AllotWidget_showPress = function _AllotWidget_showPress() {
    __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, { snapshot: true, indicator: true });
}, _AllotWidget_showChange = function _AllotWidget_showChange(change) {
    if (change.snapshot && __classPrivateFieldGet(this, _AllotWidget_expanded, "f")) {
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_render).call(this, true);
    }
    if (change.snapshot || change.indicator) {
        __classPrivateFieldGet(this, _AllotWidget_host, "f").onChange(change);
    }
    __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_scheduleTick).call(this);
}, _AllotWidget_render = function _AllotWidget_render(animate) {
    const { snapshot } = this.model;
    const text = {
        idle: snapshot.idle === null ? "" : idleText(snapshot.idle),
        nothing: NOTHING_TEXT,
    };
    const rows = listRows(snapshot, __classPrivateFieldGet(this, _AllotWidget_appIcons, "f").icon, text, this.notificationsOff);
    __classPrivateFieldGet(this, _AllotWidget_keeper, "f").show(rows, animate, (data) => __classPrivateFieldGet(this, _AllotWidget_host, "f").showRows(data, animate));
}, _AllotWidget_scheduleTick = function _AllotWidget_scheduleTick() {
    __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_clearTick).call(this);
    if (__classPrivateFieldGet(this, _AllotWidget_stopped, "f")) {
        return;
    }
    const stop = this.model.nextStopChangeMs;
    const delays = [];
    if (__classPrivateFieldGet(this, _AllotWidget_expanded, "f")) {
        delays.push(TICK_MS);
    }
    if (stop !== null) {
        delays.push(stop);
    }
    if (delays.length === 0) {
        return;
    }
    __classPrivateFieldSet(this, _AllotWidget_tick, __classPrivateFieldGet(this, _AllotWidget_host, "f").timers.setTimeout(() => {
        __classPrivateFieldSet(this, _AllotWidget_tick, null, "f");
        __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, this.model.refresh());
    }, Math.max(1, Math.ceil(Math.min(...delays)))), "f");
}, _AllotWidget_clearTick = function _AllotWidget_clearTick() {
    if (__classPrivateFieldGet(this, _AllotWidget_tick, "f") !== null) {
        __classPrivateFieldGet(this, _AllotWidget_host, "f").timers.clearTimeout(__classPrivateFieldGet(this, _AllotWidget_tick, "f"));
        __classPrivateFieldSet(this, _AllotWidget_tick, null, "f");
    }
};
/**
 * @param {IdleLine} idle
 * @returns {string}
 */
export function idleText(idle) {
    return [idle.label, idle.ids, idle.usage].filter((part) => part !== "").join("  ");
}
/**
 * @param {JobGroupKind} kind
 * @returns {string}
 */
export function groupIcon(kind) {
    return GROUP_ICONS[kind];
}
