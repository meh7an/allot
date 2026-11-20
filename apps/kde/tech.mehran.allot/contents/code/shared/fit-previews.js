// Generated from apps/gnome/allot@mehran.tech/lib/fit-previews.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
import { CancelledError } from "./client.js";
import { SERVICE_STOPPED_TEXT } from "./live-text.js";
import { SAVE_DELAY_MS } from "./settings-session.js";
/**
 * @import { FitPreview, SizeFit } from "../contract.g.mjs"
 * @import { Timers } from "./poller.js"
 * @import { SettingsSession } from "./settings-session.js"
 */
/** Why a live part shows nothing yet, before the first preview arrives. */
export const NOT_YET_TEXT = "Can't work this out yet. Trying again in a moment.";
/**
 * allotd's fit preview for the values on screen, unsaved edits included: asked 600 ms after the
 * session last changed, while a page that shows it is open in a visible window.
 */
export class FitPreviews {
    /**
     * @param {SettingsSession} session
     * @param {Timers} timers
     */
    constructor(session, timers) {
        /**
         * The latest preview, or `null` before the first one.
         *
         * @type {FitPreview | null}
         */
        this.latest = null;
        /** @type {(() => void) | null} */
        this.onChange = null;
        /** @type {unknown} */
        this._debounce = null;
        this._request = 0;
        this._active = false;
        this._session = session;
        this._timers = timers;
    }
    /** Why no preview can be shown now, or `null` when `latest` applies. */
    get unavailable() {
        const status = this._session.status;
        return status === "ready" || status === "loading" ? null : SERVICE_STOPPED_TEXT;
    }
    /**
     * The fit of a class or preset by name, or `null` while none is known.
     *
     * @param {string} table `classes` or `presets`.
     * @param {string} name
     * @returns {SizeFit | null}
     */
    fit(table, name) {
        var _a;
        if (this.unavailable !== null || this.latest === null) {
            return null;
        }
        const fits = table === "classes" ? this.latest.classes : this.latest.presets;
        return (_a = fits.find((fit) => fit.name === name)) !== null && _a !== void 0 ? _a : null;
    }
    /**
     * Starts or stops previewing; starting asks at once.
     *
     * @param {boolean} active
     */
    setActive(active) {
        this._active = active;
        this._cancelDebounce();
        if (active) {
            void this.ask();
        }
    }
    /** The session changed: asks again once it has been quiet for the save delay. */
    sessionChanged() {
        if (!this._active) {
            return;
        }
        this._cancelDebounce();
        this._debounce = this._timers.setTimeout(() => {
            this._debounce = null;
            void this.ask();
        }, SAVE_DELAY_MS);
    }
    /**
     * Asks allotd now; an answer to an older request is dropped.
     *
     * @returns {Promise<void>}
     */
    ask() {
        return __awaiter(this, void 0, void 0, function* () {
            var _a;
            if (!this._active || this._session.status !== "ready") {
                return;
            }
            this._request += 1;
            const asked = this._request;
            let reply;
            try {
                reply = yield this._session.preview();
            }
            catch (error) {
                if (error instanceof CancelledError) {
                    return;
                }
                throw error;
            }
            if (asked !== this._request || !reply.ok) {
                return;
            }
            this.latest = reply.value;
            (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this);
        });
    }
    /** Stops asking; answers that arrive later are dropped. */
    dispose() {
        this._active = false;
        this._request += 1;
        this._cancelDebounce();
    }
    _cancelDebounce() {
        if (this._debounce !== null) {
            this._timers.clearTimeout(this._debounce);
            this._debounce = null;
        }
    }
}
