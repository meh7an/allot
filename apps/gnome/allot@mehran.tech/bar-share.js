import St from "gi://St";

import { barMove } from "./lib/bar-move.js";

/** @typedef {{ duration: number, mode: number }} BarGlide */

/**
 * A bar's share of its width, eased through an adjustment, since a drawing has no animatable
 * property. The signal is tied to the drawing: a bar still easing when it is destroyed must not
 * repaint.
 */
export class BarShare {
  #actor;
  #adjustment;
  #target = 0;

  /** @param {St.DrawingArea} actor */
  constructor(actor) {
    this.#actor = actor;
    this.#adjustment = new St.Adjustment({ actor, lower: 0, upper: 1, value: 0 });
    this.#adjustment.connectObject("notify::value", () => actor.queue_repaint(), actor);
  }

  get value() {
    return this.#adjustment.value;
  }

  /**
   * @param {number} target
   * @param {BarGlide | null} glide How it may glide; `null` moves at once.
   */
  moveTo(target, glide) {
    const move = barMove(this.#target, target, this.#actor.width);
    if (move === "stay") {
      return;
    }
    this.#target = target;
    if (move === "glide" && glide !== null) {
      this.#adjustment.ease(target, glide);
    } else {
      this.#adjustment.remove_transition("value");
      this.#adjustment.value = target;
    }
  }
}
