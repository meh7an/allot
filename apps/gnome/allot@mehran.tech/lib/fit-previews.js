import { CancelledError } from "./results.js";
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
   * The latest preview, or `null` before the first one.
   *
   * @type {FitPreview | null}
   */
  latest = null;
  /** @type {(() => void) | null} */
  onChange = null;

  #session;
  #timers;
  /** @type {unknown} */
  #debounce = null;
  #request = 0;
  #active = false;

  /**
   * @param {SettingsSession} session
   * @param {Timers} timers
   */
  constructor(session, timers) {
    this.#session = session;
    this.#timers = timers;
  }

  /** Why no preview can be shown now, or `null` when `latest` applies. */
  get unavailable() {
    const status = this.#session.status;
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
    if (this.unavailable !== null || this.latest === null) {
      return null;
    }
    const fits = table === "classes" ? this.latest.classes : this.latest.presets;
    return fits.find((fit) => fit.name === name) ?? null;
  }

  /**
   * Starts or stops previewing; starting asks at once.
   *
   * @param {boolean} active
   */
  setActive(active) {
    this.#active = active;
    this.#cancelDebounce();
    if (active) {
      void this.ask();
    }
  }

  /** The session changed: asks again once it has been quiet for the save delay. */
  sessionChanged() {
    if (!this.#active) {
      return;
    }
    this.#cancelDebounce();
    this.#debounce = this.#timers.setTimeout(() => {
      this.#debounce = null;
      void this.ask();
    }, SAVE_DELAY_MS);
  }

  /**
   * Asks allotd now; an answer to an older request is dropped.
   *
   * @returns {Promise<void>}
   */
  async ask() {
    if (!this.#active || this.#session.status !== "ready") {
      return;
    }
    this.#request += 1;
    const asked = this.#request;
    let reply;
    try {
      reply = await this.#session.preview();
    } catch (error) {
      if (error instanceof CancelledError) {
        return;
      }
      throw error;
    }
    if (asked !== this.#request || !reply.ok) {
      return;
    }
    this.latest = reply.value;
    this.onChange?.();
  }

  /** Stops asking; answers that arrive later are dropped. */
  dispose() {
    this.#active = false;
    this.#request += 1;
    this.#cancelDebounce();
  }

  #cancelDebounce() {
    if (this.#debounce !== null) {
      this.#timers.clearTimeout(this.#debounce);
      this.#debounce = null;
    }
  }
}
