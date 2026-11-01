// Generated from apps/gnome/allot@mehran.tech/lib/state-clock.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/** @import { UiState } from "../contract.g.mjs" */
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
var _StateClock_now, _StateClock_generatedAtMs, _StateClock_receivedAt;
/**
 * The daemon's time between polls: the latest state's `generated_at_ms` plus the time since it
 * arrived, so that elapsed and waited times tick every second without a new poll and without
 * trusting the local clock to match allotd's.
 */
export class StateClock {
    /** @param {() => number} now The local clock in milliseconds; the tests pass their own. */
    constructor(now) {
        _StateClock_now.set(this, void 0);
        _StateClock_generatedAtMs.set(this, 0);
        /** @type {number | null} */
        _StateClock_receivedAt.set(this, null);
        __classPrivateFieldSet(this, _StateClock_now, now, "f");
    }
    /**
     * The current moment in the daemon's clock, or `null` while no live state is known.
     *
     * @returns {number | null}
     */
    get nowMs() {
        if (__classPrivateFieldGet(this, _StateClock_receivedAt, "f") === null) {
            return null;
        }
        return __classPrivateFieldGet(this, _StateClock_generatedAtMs, "f") + Math.max(0, Math.floor(__classPrivateFieldGet(this, _StateClock_now, "f").call(this) - __classPrivateFieldGet(this, _StateClock_receivedAt, "f")));
    }
    /**
     * Takes the state of a poll; `null` stops the clock, as last known times stand still.
     *
     * @param {UiState | null} state
     */
    received(state) {
        if (state === null) {
            __classPrivateFieldSet(this, _StateClock_receivedAt, null, "f");
            return;
        }
        __classPrivateFieldSet(this, _StateClock_generatedAtMs, state.generated_at_ms, "f");
        __classPrivateFieldSet(this, _StateClock_receivedAt, __classPrivateFieldGet(this, _StateClock_now, "f").call(this), "f");
    }
}
_StateClock_now = new WeakMap(), _StateClock_generatedAtMs = new WeakMap(), _StateClock_receivedAt = new WeakMap();
