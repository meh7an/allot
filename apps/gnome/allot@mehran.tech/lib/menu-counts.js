/** @import { MeterToggles, NotificationToggles } from "./settings.js" */

// The counts of the More options menu's Meters and Notifications items.

/**
 * For example `4 of 5`; Drive activity is part of Drives, so it does not count.
 *
 * @param {MeterToggles} toggles
 * @returns {string}
 */
export function meterCount(toggles) {
  return countOf([toggles.cpu, toggles.ram, toggles.gpu, toggles.vram, toggles.drives]);
}

/**
 * For example `3 of 5`. Do Not Disturb does not change it, because it only hides the banners:
 * notifications still reach the message list.
 *
 * @param {NotificationToggles} toggles
 * @returns {string}
 */
export function notificationCount(toggles) {
  return countOf([
    toggles.service_stopped,
    toggles.running_long,
    toggles.benchmark_finished,
    toggles.job_finished,
    toggles.killed_outside_flyout,
  ]);
}

/**
 * @param {boolean[]} toggles
 * @returns {string}
 */
function countOf(toggles) {
  return `${toggles.filter((on) => on).length} of ${toggles.length}`;
}
