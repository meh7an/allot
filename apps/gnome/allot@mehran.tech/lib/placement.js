/**
 * @typedef {object} Rect
 * @property {number} x
 * @property {number} y
 * @property {number} width
 * @property {number} height
 */

/**
 * @typedef {object} Point
 * @property {number} x
 * @property {number} y
 */

/** The popup's tallest size, as on Windows and macOS. */
export const MAX_HEIGHT = 640;

/** The share of the work area's height the popup may take on a small screen. */
export const MAX_SCREEN_SHARE = 0.7;

/** The gap between the panel and the popup, and the popup's distance from screen edges. */
export const MARGIN = 6;

/**
 * @param {Rect} workArea The monitor's work area, without the panel.
 * @returns {number}
 */
export function maxHeight(workArea) {
  return Math.floor(Math.min(MAX_HEIGHT, workArea.height * MAX_SCREEN_SHARE));
}

/**
 * Under the panel button, centred on it like the shell's own menus, and kept inside the work
 * area.
 *
 * @param {Rect} button The panel button in stage coordinates.
 * @param {{ width: number, height: number }} popup
 * @param {Rect} workArea
 * @returns {Point}
 */
export function anchoredPosition(button, popup, workArea) {
  const centred = {
    x: button.x + (button.width - popup.width) / 2,
    y: Math.max(button.y + button.height, workArea.y) + MARGIN,
  };
  return clampInto(centred, popup, workArea);
}

/**
 * Keeps a dragged popup whole on its monitor.
 *
 * @param {Point} position
 * @param {{ width: number, height: number }} popup
 * @param {Rect} area
 * @returns {Point}
 */
export function clampInto(position, popup, area) {
  return {
    x: Math.round(clamp(position.x, area.x + MARGIN, area.x + area.width - popup.width - MARGIN)),
    y: Math.round(clamp(position.y, area.y, area.y + area.height - popup.height - MARGIN)),
  };
}

/**
 * Like `Math.min(Math.max(value, low), high)`, except that `low` wins when the range is empty,
 * so a popup larger than its area keeps its top left corner on screen.
 *
 * @param {number} value
 * @param {number} low
 * @param {number} high
 * @returns {number}
 */
function clamp(value, low, high) {
  return Math.max(low, Math.min(value, high));
}
