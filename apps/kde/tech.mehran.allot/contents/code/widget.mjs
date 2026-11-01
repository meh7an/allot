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
var _AllotWidget_instances, _AllotWidget_host, _AllotWidget_runner, _AllotWidget_transport, _AllotWidget_keeper, _AllotWidget_appIcons, _AllotWidget_poller, _AllotWidget_lastStatus, _AllotWidget_tick, _AllotWidget_expanded, _AllotWidget_stopped, _AllotWidget_show, _AllotWidget_showPress, _AllotWidget_showChange, _AllotWidget_render, _AllotWidget_scheduleTick, _AllotWidget_clearTick;
import { DaemonClient } from "./shared/client.js";
import { DEFAULT_PORT, dataPaths, portFromToml } from "./shared/config.js";
import { accessibleName, iconFile } from "./shared/indicator-status.js";
import { DaemonPoller } from "./shared/poller.js";
import { PopupModel } from "./shared/popup-model.js";
import { AppIcons } from "./app-icons.js";
import { CommandRunner, XhrTransport, localPath, readTextFile } from "./io.js";
import { RowKeeper, listRows } from "./rows.js";
/** @import { PollResult, ServiceStatus } from "./shared/client.js" */
/** @import { DataPaths } from "./shared/config.js" */
/** @import { Timers } from "./shared/poller.js" */
/** @import { ModelChange } from "./shared/popup-model.js" */
/** @import { IdleLine } from "./shared/snapshot.js" */
/** @import { JobGroupKind } from "./shared/snapshot.js" */
/** @import { XhrFactory } from "./io.js" */
/** @import { Row, RowModel } from "./rows.js" */
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
// Times on the cards tick once a second while the popup is open.
const TICK_MS = 1000;
/**
 * What the widget needs from its QML item.
 *
 * @typedef {object} WidgetHost
 * @property {string} homeUrl `StandardPaths`' home location.
 * @property {(command: string) => void} connectSource The executable data source's
 *   `connectSource`; its `onNewData` calls `deliver`.
 * @property {XhrFactory} requests
 * @property {Timers} timers
 * @property {RowModel} rows The ListModel the popup's list shows.
 * @property {(data: Record<string, Row>, animate: boolean) => void} showRows
 *   Hands the views their rows, by key; `animate` is off when the popup opens.
 * @property {(change: ModelChange) => void} onChange Called after the model changed.
 * @property {(line: string) => void} log
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
        _AllotWidget_host.set(this, void 0);
        _AllotWidget_runner.set(this, void 0);
        _AllotWidget_transport.set(this, void 0);
        _AllotWidget_keeper.set(this, void 0);
        _AllotWidget_appIcons.set(this, void 0);
        /** @type {DaemonPoller | null} */
        _AllotWidget_poller.set(this, null);
        /** @type {ServiceStatus | null} */
        _AllotWidget_lastStatus.set(this, null);
        /** @type {unknown} */
        _AllotWidget_tick.set(this, null);
        _AllotWidget_expanded.set(this, false);
        _AllotWidget_stopped.set(this, false);
        __classPrivateFieldSet(this, _AllotWidget_host, host, "f");
        this.model = new PopupModel(host.now);
        this.model.onChange = (change) => __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, change);
        this.model.onStopAnswered = () => { var _a; return (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.pollNow(); };
        this.paths = dataPaths(localPath(host.homeUrl));
        __classPrivateFieldSet(this, _AllotWidget_runner, new CommandRunner(host.connectSource), "f");
        __classPrivateFieldSet(this, _AllotWidget_transport, new XhrTransport(host.requests, host.timers), "f");
        __classPrivateFieldSet(this, _AllotWidget_keeper, new RowKeeper(host.rows), "f");
        __classPrivateFieldSet(this, _AllotWidget_appIcons, new AppIcons(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), () => {
            __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, { snapshot: true, indicator: false });
        }), "f");
    }
    /**
     * Reads the port once, as the other apps do at launch, then polls until `stop`.
     *
     * @returns {Promise<void>}
     */
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            const toml = yield readTextFile(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), this.paths.config);
            if (__classPrivateFieldGet(this, _AllotWidget_stopped, "f")) {
                return;
            }
            const client = new DaemonClient({
                port: toml === null ? DEFAULT_PORT : portFromToml(toml),
                send: __classPrivateFieldGet(this, _AllotWidget_transport, "f").send,
                readToken: () => readTextFile(__classPrivateFieldGet(this, _AllotWidget_runner, "f"), this.paths.token),
            });
            this.model.stopper = client;
            const poller = new DaemonPoller(client, __classPrivateFieldGet(this, _AllotWidget_host, "f").timers);
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
     * rows without animating them; closing takes back any Kill?.
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
        }
        else {
            this.model.cancelConfirms();
            __classPrivateFieldGet(this, _AllotWidget_appIcons, "f").forget();
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
}
_AllotWidget_host = new WeakMap(), _AllotWidget_runner = new WeakMap(), _AllotWidget_transport = new WeakMap(), _AllotWidget_keeper = new WeakMap(), _AllotWidget_appIcons = new WeakMap(), _AllotWidget_poller = new WeakMap(), _AllotWidget_lastStatus = new WeakMap(), _AllotWidget_tick = new WeakMap(), _AllotWidget_expanded = new WeakMap(), _AllotWidget_stopped = new WeakMap(), _AllotWidget_instances = new WeakSet(), _AllotWidget_show = function _AllotWidget_show(result) {
    var _a;
    var _b, _c, _d;
    if (result.status !== __classPrivateFieldGet(this, _AllotWidget_lastStatus, "f")) {
        __classPrivateFieldSet(this, _AllotWidget_lastStatus, result.status, "f");
        const detail = (_c = (_b = result.version) !== null && _b !== void 0 ? _b : result.detail) !== null && _c !== void 0 ? _c : "";
        __classPrivateFieldGet(this, _AllotWidget_host, "f").log(`Allot: service ${result.status}${detail === "" ? "" : `: ${detail}`}`);
    }
    __classPrivateFieldGet(this, _AllotWidget_instances, "m", _AllotWidget_showChange).call(this, this.model.apply(result, (_d = (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.lastKnown) !== null && _d !== void 0 ? _d : null));
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
    const rows = listRows(snapshot, __classPrivateFieldGet(this, _AllotWidget_appIcons, "f").icon, text);
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
