// Generated from apps/gnome/allot@mehran.tech/lib/state-clock.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/** @import { UiState } from "../contract.g.mjs" */
/**
 * The daemon's time between polls: the latest state's `generated_at_ms` plus the time since it
 * arrived, so that elapsed and waited times tick every second without a new poll and without
 * trusting the local clock to match allotd's.
 */
export class StateClock {
    /** @param {() => number} now The local clock in milliseconds; the tests pass their own. */
    constructor(now) {
        this._generatedAtMs = 0;
        /** @type {number | null} */
        this._receivedAt = null;
        this._now = now;
    }
    /**
     * The current moment in the daemon's clock, or `null` while no live state is known.
     *
     * @returns {number | null}
     */
    get nowMs() {
        if (this._receivedAt === null) {
            return null;
        }
        return this._generatedAtMs + Math.max(0, Math.floor(this._now() - this._receivedAt));
    }
    /**
     * Takes the state of a poll; `null` stops the clock, as last known times stand still.
     *
     * @param {UiState | null} state
     */
    received(state) {
        if (state === null) {
            this._receivedAt = null;
            return;
        }
        this._generatedAtMs = state.generated_at_ms;
        this._receivedAt = this._now();
    }
}
