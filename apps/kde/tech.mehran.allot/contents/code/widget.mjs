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
var _AllotWidget_instances, _AllotWidget_host, _AllotWidget_runner, _AllotWidget_transport, _AllotWidget_poller, _AllotWidget_lastStatus, _AllotWidget_expanded, _AllotWidget_stopped, _AllotWidget_show;
import { DaemonClient } from "./shared/client.js";
import { DEFAULT_PORT, dataPaths, portFromToml } from "./shared/config.js";
import { accessibleName, iconFile } from "./shared/indicator-status.js";
import { DaemonPoller } from "./shared/poller.js";
import { PopupModel } from "./shared/popup-model.js";
import { CommandRunner, XhrTransport, localPath, readTextFile } from "./io.js";
/** @import { PollResult, ServiceStatus } from "./shared/client.js" */
/** @import { DataPaths } from "./shared/config.js" */
/** @import { Timers } from "./shared/poller.js" */
/** @import { ModelChange } from "./shared/popup-model.js" */
/** @import { IdleLine, SessionStrip } from "./shared/snapshot.js" */
/** @import { JobGroupKind } from "./shared/snapshot.js" */
/** @import { XhrFactory } from "./io.js" */
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
/**
 * What the widget needs from its QML item.
 *
 * @typedef {object} WidgetHost
 * @property {string} homeUrl `StandardPaths`' home location.
 * @property {(command: string) => void} connectSource The executable data source's
 *   `connectSource`; its `onNewData` calls `deliver`.
 * @property {XhrFactory} requests
 * @property {Timers} timers
 * @property {(change: ModelChange) => void} onChange Called after a poll changed the model.
 * @property {(line: string) => void} log
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
        this.model = new PopupModel();
        _AllotWidget_host.set(this, void 0);
        _AllotWidget_runner.set(this, void 0);
        _AllotWidget_transport.set(this, void 0);
        /** @type {DaemonPoller | null} */
        _AllotWidget_poller.set(this, null);
        /** @type {ServiceStatus | null} */
        _AllotWidget_lastStatus.set(this, null);
        _AllotWidget_expanded.set(this, false);
        _AllotWidget_stopped.set(this, false);
        __classPrivateFieldSet(this, _AllotWidget_host, host, "f");
        this.paths = dataPaths(localPath(host.homeUrl));
        __classPrivateFieldSet(this, _AllotWidget_runner, new CommandRunner(host.connectSource), "f");
        __classPrivateFieldSet(this, _AllotWidget_transport, new XhrTransport(host.requests, host.timers), "f");
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
     * Follows Plasma's `expanded`: opening polls at once and starts without Keep Open.
     *
     * @param {boolean} expanded
     */
    setExpanded(expanded) {
        var _a;
        __classPrivateFieldSet(this, _AllotWidget_expanded, expanded, "f");
        if (expanded) {
            this.model.resetForOpen();
        }
        (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.setPopupOpen(expanded);
    }
    toggleKeepOpen() {
        this.model.toggleStayOpen();
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
_AllotWidget_host = new WeakMap(), _AllotWidget_runner = new WeakMap(), _AllotWidget_transport = new WeakMap(), _AllotWidget_poller = new WeakMap(), _AllotWidget_lastStatus = new WeakMap(), _AllotWidget_expanded = new WeakMap(), _AllotWidget_stopped = new WeakMap(), _AllotWidget_instances = new WeakSet(), _AllotWidget_show = function _AllotWidget_show(result) {
    var _a;
    var _b, _c, _d;
    if (result.status !== __classPrivateFieldGet(this, _AllotWidget_lastStatus, "f")) {
        __classPrivateFieldSet(this, _AllotWidget_lastStatus, result.status, "f");
        const detail = (_c = (_b = result.version) !== null && _b !== void 0 ? _b : result.detail) !== null && _c !== void 0 ? _c : "";
        __classPrivateFieldGet(this, _AllotWidget_host, "f").log(`Allot: service ${result.status}${detail === "" ? "" : `: ${detail}`}`);
    }
    const change = this.model.apply(result, (_d = (_a = __classPrivateFieldGet(this, _AllotWidget_poller, "f")) === null || _a === void 0 ? void 0 : _a.lastKnown) !== null && _d !== void 0 ? _d : null);
    if (change.snapshot || change.indicator) {
        __classPrivateFieldGet(this, _AllotWidget_host, "f").onChange(change);
    }
};
/**
 * A job row's session line: `a1b2c3d4 · app · 1.5 cores · 2.0 GB`, without empty parts.
 *
 * @param {SessionStrip} strip
 * @returns {string}
 */
export function stripText(strip) {
    return [strip.shortId, strip.project, strip.usage].filter((part) => part !== "").join(" · ");
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
