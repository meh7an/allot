// On GNOME and KDE a bar's glide runs in the shell's own process, and a slow job's bar would glide
// nearly all the time for a change nobody can see. So small moves happen in one repaint.

/** @typedef {"stay" | "jump" | "glide"} BarMove */

/** Moves of at least this many pixels glide. */
export const GLIDE_MIN_PX = 3;

/** Moves under this many pixels wait until they add up to something visible. */
const VISIBLE_PX = 0.5;

/**
 * @param {number} from The share of the bar's width it last moved to.
 * @param {number} to The share it should show now.
 * @param {number} widthPx The bar's width; 0 before it is laid out, when any share is fine.
 * @returns {BarMove}
 */
export function barMove(from, to, widthPx) {
  if (widthPx <= 0) {
    return from === to ? "stay" : "jump";
  }
  const pixels = Math.abs(to - from) * widthPx;
  if (pixels < VISIBLE_PX) {
    return "stay";
  }
  return pixels < GLIDE_MIN_PX ? "jump" : "glide";
}
