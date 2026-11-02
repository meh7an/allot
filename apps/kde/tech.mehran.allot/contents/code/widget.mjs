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
import { Announcer } from "./shared/accessible-text.js";
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
 * @property {(text: string) => void} announce Says something to a screen reader.
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
        this._announcer = new Announcer();
        /** @type {DaemonPoller | null} */
        this._poller = null;
        /** @type {ServiceStatus | null} */
        this._lastStatus = null;
        /** @type {unknown} */
        this._tick = null;
        /** @type {Promise<void>} */
        this._saving = Promise.resolve();
        this._expanded = false;
        this._stopped = false;
        this._host = host;
        this.model = new PopupModel(host.now);
        this.model.onChange = (change) => this._showChange(change);
        this.model.onStopAnswered = () => { var _a; return (_a = this._poller) === null || _a === void 0 ? void 0 : _a.pollNow(); };
        this.model.onKillSent = (lease) => this._rules.noteLocalKill(lease);
        this.model.onSettingsChanged = () => this._saveSettings();
        if (host.timeOfDay !== undefined) {
            this.model.timeOfDay = host.timeOfDay;
        }
        this.paths = dataPaths(localPath(host.homeUrl));
        this._configFolder = localPath(host.configUrl);
        this._dataFolder = localPath(host.dataUrl);
        this._runner = new CommandRunner(host.connectSource);
        this._transport = new XhrTransport(host.requests, host.timers);
        this._keeper = new RowKeeper(host.rows);
        this._appIcons = new AppIcons(this._runner, () => {
            this._showChange({ snapshot: true, indicator: false });
        });
        this._rules = new NotificationRules(host.now);
        this._notifier = new Notifier({
            send: host.sendNotification,
            onButton: (button, target) => this._pressNotificationButton(button, target),
            onActivate: (target) => this._showRow(target),
        });
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
                readTextFile(this._runner, this.paths.config),
                readTextFile(this._runner, this.paths.settings),
                copyFile(this._runner, localPath(this._host.notifyRcUrl), `${this._dataFolder}/knotifications6/${NOTIFY_RC_FILE}`),
            ]);
            if (this._stopped) {
                return;
            }
            if (!notifyRc) {
                this._host.log("Allot: cannot install the notifyrc; notifications will not show");
            }
            this._showChange(this.model.loadSettings(decodeSettings(settings)));
            this.refreshNotificationState();
            const client = new DaemonClient({
                port: toml === null ? DEFAULT_PORT : portFromToml(toml),
                send: this._transport.send,
                readToken: () => readTextFile(this._runner, this.paths.token),
            });
            this.model.stopper = client;
            const launcher = new SystemdServiceLauncher({
                home: localPath(this._host.homeUrl),
                configHome: this._configFolder,
                root: this.paths.root,
            }, serviceSystem(this._runner));
            const poller = new DaemonPoller(client, this._host.timers, Object.assign({ launcher }, (this._host.now === undefined ? {} : { now: this._host.now })));
            poller.onUpdate = (result) => this._show(result);
            poller.setPopupOpen(this._expanded);
            this._poller = poller;
            try {
                yield poller.start();
            }
            catch (error) {
                this._host.log(`Allot: polling stopped: ${error}`);
            }
        });
    }
    stop() {
        var _a;
        this._stopped = true;
        this._clearTick();
        (_a = this._poller) === null || _a === void 0 ? void 0 : _a.stop();
        this._transport.cancel();
    }
    /**
     * The executable data source's answers.
     *
     * @param {string} command
     * @param {Record<string, unknown>} data
     */
    deliver(command, data) {
        this._runner.deliver(command, data);
    }
    /**
     * Follows Plasma's `expanded`: opening polls at once, starts without Keep Open and shows the
     * rows without animating them; closing takes back any Kill? and the Stop card.
     *
     * @param {boolean} expanded
     */
    setExpanded(expanded) {
        var _a;
        this._expanded = expanded;
        if (expanded) {
            this.model.resetForOpen();
            this.model.refresh();
            this._announcer.reset();
            this._render(false);
            this.refreshNotificationState();
        }
        else {
            this.model.cancelConfirms();
            this._appIcons.forget();
            this.stopQuestion = "";
        }
        (_a = this._poller) === null || _a === void 0 ? void 0 : _a.setPopupOpen(expanded);
        this._scheduleTick();
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
        this._showPress();
    }
    /**
     * A press of Remove on a queued request.
     *
     * @param {number} lease
     */
    pressRemove(lease) {
        this.model.pressRemove(lease);
        this._showPress();
    }
    /**
     * Focus left a Kill? confirm.
     *
     * @param {number} lease
     */
    cancelConfirm(lease) {
        if (this.model.cancelConfirm(lease)) {
            this._showPress();
        }
    }
    /** Esc takes back a Kill? confirm before it closes the popup. */
    cancelConfirms() {
        if (this.model.cancelConfirms()) {
            this._showPress();
        }
    }
    /**
     * A leaving row's view has collapsed.
     *
     * @param {string} key
     */
    rowGone(key) {
        this._keeper.gone(key);
    }
    /** The Resources card's header. */
    toggleResources() {
        this.model.toggleResources();
        this._showPress();
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
        this._showPress();
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
        this._showPress();
    }
    /** The first-run card's Done. */
    finishFirstRun() {
        this.model.finishFirstRun();
        this._showPress();
    }
    /** The banner's Start Service, or the panel menu's while allotd is down. */
    startService() {
        return __awaiter(this, void 0, void 0, function* () {
            const poller = this._poller;
            if (poller === null) {
                return;
            }
            this._showChange(this.model.noteStarting());
            const problem = yield poller.startService();
            if (problem !== null) {
                this._host.log(`Allot: cannot start the service: ${problem}`);
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
                const question = stopConfirmation((_b = (_a = this._poller) === null || _a === void 0 ? void 0 : _a.lastKnown) !== null && _b !== void 0 ? _b : null);
                if (question === null) {
                    this._stopService();
                }
                else {
                    this.stopQuestion = question;
                    this._host.expand();
                    this._host.onChange({ snapshot: false, indicator: false });
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
        this._stopService();
    }
    /** The Stop card's Cancel. */
    cancelStop() {
        this.stopQuestion = "";
        this._host.onChange({ snapshot: false, indicator: false });
    }
    /**
     * Reads whether Plasma would show notifications now: when the popup or More options opens, since
     * Plasma tells the widget of neither switch.
     */
    refreshNotificationState() {
        return __awaiter(this, void 0, void 0, function* () {
            const [inhibited, events, plasma] = yield Promise.all([
                this._runner.run(INHIBITED_COMMAND),
                readTextFile(this._runner, `${this._configFolder}/${NOTIFY_RC_FILE}`),
                readTextFile(this._runner, `${this._configFolder}/plasmanotifyrc`),
            ]);
            if (this._stopped) {
                return;
            }
            const off = isOffInSettings(events, plasma);
            const doNotDisturb = inhibited.exitCode === 0 && parseInhibited(inhibited.stdout);
            if (off !== this.notificationsOff || doNotDisturb !== this.model.doNotDisturb) {
                this.notificationsOff = off;
                this.model.setDoNotDisturb(doNotDisturb);
                this._showPress();
            }
        });
    }
    /** The menu's and the first-run card's way to turn notifications back on. */
    openNotificationSettings() {
        this._runner.run(detachedCommand(SETTINGS_ARGV));
    }
    /** Opens the logs folder. */
    openLogs() {
        this._host.openUrl(`file://${this.paths.logs}`);
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
    /** @param {PollResult} result */
    _show(result) {
        var _a, _b;
        var _c, _d, _e, _f;
        if (result.status !== this._lastStatus) {
            this._lastStatus = result.status;
            const detail = (_d = (_c = result.version) !== null && _c !== void 0 ? _c : result.detail) !== null && _d !== void 0 ? _d : "";
            this._host.log(`Allot: service ${result.status}${detail === "" ? "" : `: ${detail}`}`);
        }
        const lastKnown = (_e = (_a = this._poller) === null || _a === void 0 ? void 0 : _a.lastKnown) !== null && _e !== void 0 ? _e : null;
        const startProblem = (_f = (_b = this._poller) === null || _b === void 0 ? void 0 : _b.startProblem) !== null && _f !== void 0 ? _f : null;
        this._showChange(this.model.apply(result, lastKnown, startProblem));
        this._notify(result);
        if (this.model.wantsFirstOpen) {
            this.model.noteFirstOpened();
            this._host.expand();
        }
    }
    /** @param {PollResult} result */
    _notify(result) {
        var _a;
        var _b;
        const update = this._rules.observe({
            status: result.status,
            live: result.state,
            lastKnown: (_b = (_a = this._poller) === null || _a === void 0 ? void 0 : _a.lastKnown) !== null && _b !== void 0 ? _b : null,
            toggles: this.model.settings.notifications,
            popupOpen: this._expanded,
        });
        for (const message of update.show) {
            this._notifier.show(message, this.iconFile);
            this._host.log(`Allot: notified ${message.tag}`);
        }
        for (const tag of update.withdraw) {
            this._notifier.withdraw(tag);
        }
    }
    /**
     * @param {NotificationButton} button
     * @param {string | null} target
     */
    _pressNotificationButton(button, target) {
        switch (button) {
            case "start-service":
                this.startService();
                break;
            case "open-logs":
                this.openLogs();
                break;
            case "show":
                this._showRow(target);
                break;
        }
    }
    /** @param {string | null} target */
    _showRow(target) {
        this._host.expand();
        this._host.scrollTo(target);
    }
    _stopService() {
        return __awaiter(this, void 0, void 0, function* () {
            const poller = this._poller;
            if (poller === null) {
                return;
            }
            this._rules.expectStop();
            try {
                const problem = yield poller.stopService();
                if (problem !== null) {
                    this._host.log(`Allot: cannot stop the service: ${problem}`);
                }
            }
            catch (error) {
                // Only the widget going away cancels the request.
                this._host.log(`Allot: stop not finished: ${error}`);
            }
        });
    }
    // One write at a time, in order, so a slow one never replaces a newer one.
    _saveSettings() {
        const text = encodeSettings(this.model.settings);
        this._saving = this._saving.then(() => __awaiter(this, void 0, void 0, function* () {
            if (!(yield writeTextFile(this._runner, this.paths.settings, text))) {
                this._host.log(`Allot: cannot save ${this.paths.settings}`);
            }
        }));
    }
    _showPress() {
        this._showChange({ snapshot: true, indicator: true });
    }
    /** @param {ModelChange} change */
    _showChange(change) {
        if (change.snapshot && this._expanded) {
            this._render(true);
        }
        if (change.snapshot || change.indicator) {
            this._host.onChange(change);
        }
        this._scheduleTick();
    }
    /** @param {boolean} animate */
    _render(animate) {
        const { snapshot } = this.model;
        const text = {
            idle: snapshot.idle === null ? "" : idleText(snapshot.idle),
            nothing: NOTHING_TEXT,
        };
        const rows = listRows(snapshot, this._appIcons.icon, text, this.notificationsOff);
        this._keeper.show(rows, animate, (data) => this._host.showRows(data, animate));
        for (const spoken of this._announcer.observe(snapshot)) {
            this._host.announce(spoken);
        }
    }
    /**
     * Wakes up for the next second while the popup is open, and for the next countdown second or
     * hold end either way, since a Removed card still counts for the panel icon.
     */
    _scheduleTick() {
        this._clearTick();
        if (this._stopped) {
            return;
        }
        const stop = this.model.nextStopChangeMs;
        const delays = [];
        if (this._expanded) {
            delays.push(TICK_MS);
        }
        if (stop !== null) {
            delays.push(stop);
        }
        if (delays.length === 0) {
            return;
        }
        this._tick = this._host.timers.setTimeout(() => {
            this._tick = null;
            this._showChange(this.model.refresh());
        }, Math.max(1, Math.ceil(Math.min(...delays))));
    }
    _clearTick() {
        if (this._tick !== null) {
            this._host.timers.clearTimeout(this._tick);
            this._tick = null;
        }
    }
}
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
