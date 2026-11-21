// Generated from apps/gnome/allot@mehran.tech/lib/settings-window.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
import { CancelledError } from "./results.js";
import { FitPreviews, NOT_YET_TEXT } from "./fit-previews.js";
import { benchmarksText, estimatesText, fitsCaption, fitsText, headroomText, historyText, } from "./live-text.js";
import { START_TIMEOUT_MS } from "./poller.js";
import { resourcesView } from "./resources-view.js";
import { restartService } from "./service-launcher.js";
import { STOP_CONFIRM_TITLE, stopConfirmation } from "./service-menu.js";
import { serviceButtonText, serviceRow, sessionServiceStatus } from "./service-row.js";
import { NumberModel, RuleListModel, SizeTableModel, SwitchModel, } from "./setting-values.js";
import { SERVICE_PAGE, restartNote, settingsBanners } from "./settings-banners.js";
import { SettingsAnnouncer, announcerView } from "./settings-announcer.js";
import { SettingsSession } from "./settings-session.js";
import { LOADING_TEXT, resetAllConfirmation, resetPageConfirmation, saveStatusText, } from "./settings-text.js";
/**
 * @import { PollResult } from "./results.js"
 * @import { DataPaths } from "./config.js"
 * @import { Timers } from "./poller.js"
 * @import { MeterToggles } from "./settings.js"
 * @import { RestartClient, RestartLauncher } from "./service-launcher.js"
 * @import { Reach } from "./service-row.js"
 * @import { BannerAction, SettingsBanner } from "./settings-banners.js"
 * @import { PageModel, SettingsClient } from "./settings-session.js"
 * @import { Confirmation } from "./settings-text.js"
 * @import { ValueModel } from "./setting-values.js"
 */
// The Settings window of the GNOME and KDE panels without its toolkit: the session with allotd,
// what each page shows, the live sentences, and the service's Start, Stop and Restart. Each panel
// builds its widgets from this and calls back into it.
/** allotd's pages in their order, with each panel's icon for them. */
export const ALLOTD_PAGES = [
    {
        id: "admission",
        label: "Admission",
        gnomeIcon: "preferences-system-symbolic",
        kdeIcon: "view-statistics",
    },
    { id: "sizes", label: "Sizes", gnomeIcon: "view-grid-symbolic", kdeIcon: "view-list-details" },
    {
        id: "commands",
        label: "Commands",
        gnomeIcon: "utilities-terminal-symbolic",
        kdeIcon: "utilities-terminal",
    },
    { id: "benchmarks", label: "Benchmarks", gnomeIcon: "alarm-symbolic", kdeIcon: "chronometer" },
    {
        id: "leases",
        label: "Leases",
        gnomeIcon: "document-open-recent-symbolic",
        kdeIcon: "password-copy",
    },
    {
        id: "service",
        label: "Service",
        gnomeIcon: "network-server-symbolic",
        kdeIcon: "network-server",
    },
];
/** The pages whose sentences use allotd's fit preview, which runs only while one is shown. */
export const PAGES_WITH_FITS = ["admission", "sizes"];
export const WHAT_FITS_TITLE = "What Fits on This Machine";
export const TEST_COMMAND_TITLE = "Test a Command";
export const TEST_COMMAND_HELP = "Shows how the hook would treat this command with the rules below.";
export const TEST_RULE_HELP = "Shows how the hook would treat this command with this rule.";
const PORT_KEY = "daemon.port";
const START_POLL_MS = 500;
/**
 * A part of an allotd page, in the order it shows.
 *
 * @typedef {(
 *   | { kind: "extra", id: "service_status" | "what_fits" | "files" }
 *   | { kind: "settings", title: string, settings: ValueModel<unknown>[], live: string | null }
 *   | { kind: "sizes", title: string, table: SizeTableModel }
 *   | { kind: "rules", title: string, rules: RuleListModel }
 *   | { kind: "reset", all: boolean }
 * )} PageSection
 */
/**
 * The parts of an allotd page: the spec's groups, rarely changed keys last under an Advanced
 * heading, the page's own groups where they go, then Reset.
 *
 * @param {PageModel} model
 * @returns {PageSection[]}
 */
export function pageSections(model) {
    const pageId = model.spec.id;
    /** @type {PageSection[]} */
    const sections = [];
    if (pageId === SERVICE_PAGE) {
        sections.push({ kind: "extra", id: "service_status" });
    }
    /** @type {PageSection[]} */
    const advanced = [];
    for (const group of model.groups) {
        const normal = group.settings.filter((setting) => !setting.spec.advanced);
        const rare = group.settings.filter((setting) => setting.spec.advanced);
        if (normal.length > 0) {
            const live = rare.length === 0 ? group.spec.id : null;
            sections.push(settingsSection(group.spec.label, normal, live));
        }
        if (rare.length > 0) {
            // The live sentence goes with the group's last part.
            advanced.push(settingsSection(`Advanced: ${group.spec.label}`, rare, group.spec.id));
        }
        if (pageId === "admission" && group.spec.id === "learn") {
            sections.push({ kind: "extra", id: "what_fits" });
        }
    }
    if (pageId === SERVICE_PAGE) {
        sections.push({ kind: "extra", id: "files" });
    }
    sections.push(...advanced, { kind: "reset", all: pageId === SERVICE_PAGE });
    return sections;
}
/**
 * @param {string} title
 * @param {ValueModel<unknown>[]} settings
 * @param {string | null} live
 * @returns {PageSection}
 */
function settingsSection(title, settings, live) {
    const first = settings[0];
    if (first instanceof SizeTableModel) {
        return { kind: "sizes", title, table: first };
    }
    if (first instanceof RuleListModel) {
        return { kind: "rules", title, rules: first };
    }
    return { kind: "settings", title, settings, live };
}
/**
 * What an allotd page shows above its settings.
 *
 * @typedef {object} PageState
 * @property {SettingsBanner[]} banners
 * @property {string} status The save status, Loading settings…, or empty.
 * @property {boolean} readOnly
 */
/**
 * The Service page's status row.
 *
 * @typedef {object} ServiceRowView
 * @property {string} text
 * @property {string} button The button's title.
 * @property {boolean} canPress
 * @property {boolean} opensLogs Whether Open Logs shows beside it.
 */
/**
 * What Fits on This Machine: a row per class, then the caption.
 *
 * @typedef {object} WhatFitsView
 * @property {Array<{ label: string, text: string }>} rows
 * @property {string} caption
 */
/**
 * What the window model needs from the panel that shows it.
 *
 * @typedef {object} WindowHost
 * @property {(confirmation: Confirmation, onConfirm: () => void) => void} confirm Asks first;
 *   the primary button is destructive.
 * @property {(text: string) => void} notify A brief notice.
 * @property {(text: string) => void} [log] A line for the panel's log, for what the user need not
 *   see.
 * @property {(path: string) => void} openPath Opens a file or folder in its default app.
 * @property {() => number} now A monotonic clock in milliseconds.
 */
/**
 * @typedef {object} WindowOptions
 * @property {SettingsClient & RestartClient & {
 *   poll: () => Promise<PollResult>,
 *   shutdown: (options?: { record: boolean }) => Promise<string | null>,
 * }} client
 * @property {Timers} timers
 * @property {RestartLauncher} launcher
 * @property {DataPaths} paths
 * @property {Reach} reach How the panel reaches allotd, which its texts name when it does not
 *   answer.
 * @property {(port: number) => void} [followPort] Called after a restart that moved allotd to a
 *   new port, for a panel that dials one.
 * @property {(value: number) => string} number Formats a number for the user's locale.
 * @property {() => Promise<string | null>} readStopRecord
 * @property {WindowHost} host
 * @property {() => Promise<string | null>} [stop] Stops the service, when the panel has its
 *   own way that its popup follows; `client.shutdown()` otherwise.
 * @property {() => Promise<string | null>} [start] Starts it likewise; `launcher.start()`
 *   otherwise.
 */
/**
 * One Settings window's state: its session with allotd, the fit previews, and the service's
 * state as the window knows it. `onChange` fires whenever anything it shows may have changed.
 */
export class SettingsWindowModel {
    /** @param {WindowOptions} options */
    constructor(options) {
        var _a, _b, _c;
        /** Whether a GPU shares system memory, so the VRAM meter reads "GPU memory". */
        this.sharesMemory = false;
        /** @type {(() => void) | null} */
        this.onChange = null;
        /**
         * Speaks a text to screen readers without moving focus: errors and outcomes.
         *
         * @type {((text: string) => void) | null}
         */
        this.onAnnounce = null;
        this._announcer = new SettingsAnnouncer();
        /** @type {string | null} */
        this._shownPage = null;
        /** @type {string | null} */
        this._version = null;
        this._starting = false;
        this._startedAt = 0;
        /** @type {unknown} */
        this._startTimer = null;
        this._disposed = false;
        this._client = options.client;
        this._timers = options.timers;
        this._launcher = options.launcher;
        this._host = options.host;
        this._reach = options.reach;
        this._followPort = (_a = options.followPort) !== null && _a !== void 0 ? _a : null;
        this._stop = (_b = options.stop) !== null && _b !== void 0 ? _b : (() => options.client.shutdown());
        this._start = (_c = options.start) !== null && _c !== void 0 ? _c : (() => options.launcher.start());
        this.number = options.number;
        this.paths = options.paths;
        this.session = new SettingsSession(options.client, options.timers, {
            platform: "linux",
            readStopRecord: options.readStopRecord,
        });
        this.fits = new FitPreviews(this.session, options.timers);
        let previousStatus = this.session.status;
        this.session.onChange = () => {
            if (this.session.status !== previousStatus) {
                previousStatus = this.session.status;
                this._statusChanged();
            }
            this.fits.sessionChanged();
            this._changed();
        };
        this.fits.onChange = () => this._changed();
    }
    /** @returns {Reach} */
    get reach() {
        return this._reach;
    }
    /**
     * Fetches now and every 5 s while shown; hiding sends what is pending.
     *
     * @param {boolean} visible
     */
    setVisible(visible) {
        this.session.setVisible(visible);
    }
    /** Stops the timers; a change still pending was sent by `setVisible(false)`. */
    dispose() {
        this._disposed = true;
        this.session.onChange = null;
        this.fits.onChange = null;
        this.fits.dispose();
        this.session.dispose();
        if (this._startTimer !== null) {
            this._timers.clearTimeout(this._startTimer);
            this._startTimer = null;
        }
    }
    /**
     * The page now shown: fit previews run only while a page that shows them is open.
     *
     * @param {string} pageId
     */
    showPage(pageId) {
        this.fits.setActive(PAGES_WITH_FITS.includes(pageId));
        if (pageId !== this._shownPage) {
            this._shownPage = pageId;
            this._announcer.pageShown();
            this._announce();
        }
    }
    /** Fetches the settings again, such as when the window takes focus. */
    refresh() {
        return __awaiter(this, void 0, void 0, function* () {
            try {
                yield this.session.refresh();
            }
            catch (error) {
                if (!(error instanceof CancelledError)) {
                    throw error;
                }
            }
        });
    }
    /**
     * Whether a GPU shares system memory, from one poll of allotd.
     *
     * @param {MeterToggles} meters
     */
    readSharesMemory(meters) {
        return __awaiter(this, void 0, void 0, function* () {
            try {
                const result = yield this._client.poll();
                if (result.state !== null) {
                    this.sharesMemory = resourcesView("running", result.state, meters, false).sharesMemory;
                    this._changed();
                }
            }
            catch (error) {
                if (!(error instanceof CancelledError)) {
                    throw error;
                }
            }
        });
    }
    /**
     * @param {string} pageId
     * @returns {PageState}
     */
    pageState(pageId) {
        const session = this.session;
        const save = session.saveStatus;
        // While allotd does not answer, "All changes saved" would say more than is known.
        const quiet = session.status !== "ready" && save.kind === "saved";
        const status = session.status === "loading" ? LOADING_TEXT : quiet ? "" : saveStatusText(save);
        return {
            banners: settingsBanners(session, pageId, this._reach, this.number),
            status,
            readOnly: session.readOnly,
        };
    }
    /**
     * A group's live sentence, or `null` for a group without one.
     *
     * @param {string} pageId
     * @param {string} groupId
     * @returns {string | null}
     */
    live(pageId, groupId) {
        const number = this.number;
        /** @param {string} key */
        const value = (key) => {
            var _a;
            const model = this.session.setting(key);
            return model instanceof NumberModel ? ((_a = model.value) !== null && _a !== void 0 ? _a : 0) : 0;
        };
        switch (`${pageId}/${groupId}`) {
            case "admission/keep_free":
                if (this.fits.unavailable !== null) {
                    return this.fits.unavailable;
                }
                return this.fits.latest === null
                    ? NOT_YET_TEXT
                    : headroomText(this.fits.latest.kept_free, number);
            case "admission/learn": {
                const lower = this.session.setting("estimates.lower");
                return estimatesText({
                    lower: lower instanceof SwitchModel ? lower.value : true,
                    minRuns: value("estimates.min_runs"),
                    marginPct: value("estimates.margin_pct"),
                }, number);
            }
            case "benchmarks/quiet":
                return benchmarksText({
                    cpuPct: value("benchmark.quiet_cpu_pct"),
                    gpuPct: value("benchmark.quiet_gpu_pct"),
                    quietSecs: value("benchmark.quiet_secs"),
                    maxWaitSecs: value("benchmark.max_quiet_wait_secs"),
                }, number);
            case "leases/history":
                return historyText({
                    leaseDays: value("retention.lease_days"),
                    historyDays: value("retention.history_days"),
                    keepRuns: value("retention.keep_runs_per_job"),
                    maxRuns: value("retention.max_job_runs"),
                }, number);
            default:
                return null;
        }
    }
    /**
     * An extra line under a setting, such as Port's restart note.
     *
     * @param {string} key
     * @returns {string | null}
     */
    note(key) {
        return restartNote(this.session, key);
    }
    /** @returns {WhatFitsView} */
    get whatFits() {
        var _a, _b;
        const preview = this.fits.unavailable === null ? this.fits.latest : null;
        const classes = this.session.setting("classes");
        const labels = classes instanceof SizeTableModel ? classes.rows : [];
        const rows = ((_a = preview === null || preview === void 0 ? void 0 : preview.classes) !== null && _a !== void 0 ? _a : []).map((fit) => {
            var _a;
            var _b;
            const label = (_b = (_a = labels.find((view) => view.row.name === fit.name)) === null || _a === void 0 ? void 0 : _a.label) !== null && _b !== void 0 ? _b : fit.name;
            const now = fitsText(fit.now, fit.min_cores, this.number);
            const idle = fitsText(fit.idle, fit.min_cores, this.number);
            return { label, text: `Now: ${now} · When idle: ${idle}` };
        });
        const caption = (_b = this.fits.unavailable) !== null && _b !== void 0 ? _b : (preview === null ? NOT_YET_TEXT : fitsCaption(preview.cpu_busy_pct, this.number));
        return { rows, caption };
    }
    /** @returns {ServiceRowView | null} `null` before allotd first answered. */
    get serviceRow() {
        const status = sessionServiceStatus(this.session.status, this._starting);
        if (status === null) {
            return null;
        }
        const shown = serviceRow(status, this.session.status, this._version, this._reach);
        return {
            text: shown.text,
            button: serviceButtonText(shown.button),
            canPress: shown.button !== "starting",
            opensLogs: shown.opensLogs,
        };
    }
    /**
     * The changed settings Reset Page would reset; none while they cannot be changed.
     *
     * @param {string} pageId
     * @returns {ValueModel<unknown>[]}
     */
    changedOn(pageId) {
        return this.session.readOnly ? [] : this.session.changedOn(pageId);
    }
    /** @returns {boolean} Whether Reset All Service Settings… has anything to reset. */
    get canResetAll() {
        return !this.session.readOnly && this.session.changedSettings().length > 0;
    }
    /**
     * Reset Page…, after asking.
     *
     * @param {string} pageId
     * @param {string} label
     */
    resetPage(pageId, label) {
        const changed = this.changedOn(pageId);
        if (changed.length > 0) {
            this._host.confirm(resetPageConfirmation(label, changed, this.number), () => this.session.resetChanged(pageId));
        }
    }
    /** Reset All Service Settings…, after asking. */
    resetAll() {
        const changed = this.session.changedSettings();
        if (changed.length > 0) {
            this._host.confirm(resetAllConfirmation(changed.length), () => this.session.resetChanged(null));
        }
    }
    /**
     * A banner's button.
     *
     * @param {BannerAction} action
     */
    act(action) {
        switch (action) {
            case "start_service":
                void this._startService();
                break;
            case "restart_service":
                void this._restartService();
                break;
            case "try_now":
            case "check_again":
                void this.refresh();
                break;
            case "open_logs":
                this.openLogs();
                break;
            case "open_file":
                this.openFile();
                break;
            case "apply_mine":
                this.session.applyMine();
                break;
            case "keep_file_values":
                this.session.keepFileValues();
                break;
            case "save_again":
                void this.session.flush();
                break;
        }
    }
    /** The Service row's button: Stop Service… or Start Service. */
    pressService() {
        const status = sessionServiceStatus(this.session.status, this._starting);
        if (status === "running" || status === "incompatible") {
            void this._stopService();
        }
        else if (status === "stopped") {
            void this._startService();
        }
    }
    openLogs() {
        this._host.openPath(this.paths.logs);
    }
    openFile() {
        this._host.openPath(this.paths.config);
    }
    _changed() {
        var _a;
        if (!this._disposed) {
            (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this);
            this._announce();
        }
    }
    /** Speaks what changed since the last call, for the page shown. */
    _announce() {
        var _a;
        if (this._shownPage === null) {
            return;
        }
        const banners = settingsBanners(this.session, this._shownPage, this._reach, this.number);
        const text = this._announcer.update(announcerView(this.session, banners, this.number));
        if (text !== "") {
            (_a = this.onAnnounce) === null || _a === void 0 ? void 0 : _a.call(this, text);
        }
    }
    /** The session started or stopped answering: a start under way ends, the version is read. */
    _statusChanged() {
        if (this.session.status === "ready") {
            this._starting = false;
            this._client
                .healthVersion()
                .then((version) => {
                this._version = version;
                this._changed();
            })
                .catch((error) => {
                var _a, _b;
                // The version is only a label, so a failed read does not stop the window.
                if (!(error instanceof CancelledError)) {
                    (_b = (_a = this._host).log) === null || _b === void 0 ? void 0 : _b.call(_a, `Allot: cannot read the service version: ${error}`);
                }
            });
        }
    }
    /**
     * A brief notice, unless the window is gone: its host may have destroyed what shows it.
     *
     * @param {string} text
     */
    _notify(text) {
        if (!this._disposed) {
            this._host.notify(text);
        }
    }
    /** Stop asks only while leases are active; the stop lasts until a start. */
    _stopService() {
        return __awaiter(this, void 0, void 0, function* () {
            var _a;
            const poll = yield this._client.poll().catch(() => null);
            const question = stopConfirmation((_a = poll === null || poll === void 0 ? void 0 : poll.state) !== null && _a !== void 0 ? _a : null);
            const stop = () => __awaiter(this, void 0, void 0, function* () {
                const problem = yield this._stop();
                if (problem !== null) {
                    this._notify(`The service did not stop: ${problem}`);
                }
                yield this.refresh();
            });
            if (question === null) {
                yield stop();
                return;
            }
            this._host.confirm({ title: STOP_CONFIRM_TITLE, body: question, button: "Stop Service" }, () => void stop());
        });
    }
    _startService() {
        return __awaiter(this, void 0, void 0, function* () {
            this._beginStarting();
            const problem = yield this._start();
            if (problem !== null) {
                this._starting = false;
                this._notify(`The service did not start: ${problem}`);
            }
            this._changed();
        });
    }
    /** For a new port or a refused key: through systemd when its unit runs allotd. */
    _restartService() {
        return __awaiter(this, void 0, void 0, function* () {
            var _a, _b;
            this._beginStarting();
            const saved = (_a = this.session.setting(PORT_KEY)) === null || _a === void 0 ? void 0 : _a.saved;
            const problem = yield restartService(this._client, this._launcher, this._timers);
            if (problem === null && this._reach.kind === "port" && typeof saved === "number") {
                // The restarted allotd listens on the port now in config.toml.
                this._reach = { kind: "port", port: saved };
                (_b = this._followPort) === null || _b === void 0 ? void 0 : _b.call(this, saved);
            }
            if (problem !== null) {
                this._starting = false;
                this._notify(`The service did not restart: ${problem}`);
            }
            this._changed();
        });
    }
    /** Shows Starting… and asks allotd often until it answers or the start times out. */
    _beginStarting() {
        this._starting = true;
        this._startedAt = this._host.now();
        this._changed();
        const tick = () => {
            this._startTimer = null;
            if (this._disposed || !this._starting) {
                return;
            }
            if (this._host.now() - this._startedAt > START_TIMEOUT_MS) {
                this._starting = false;
                this._changed();
                return;
            }
            void this.refresh().then(() => {
                if (!this._disposed) {
                    this._startTimer = this._timers.setTimeout(tick, START_POLL_MS);
                }
            });
        };
        if (this._startTimer !== null) {
            this._timers.clearTimeout(this._startTimer);
        }
        this._startTimer = this._timers.setTimeout(tick, START_POLL_MS);
    }
}
