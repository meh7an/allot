import { defaultSettings } from "./settings.js";
import { onOff } from "./settings-text.js";

/** @import { TraySettings } from "./settings.js" */

// The Settings window's Panel page on GNOME and KDE: the Meters and Notifications groups over the
// toggles in `tray.json` (DESIGN.md section 4, Panel), and its Reset Page. There is no Startup
// group: the desktop starts the extension or widget.

/**
 * One switch on the Panel page.
 *
 * @typedef {object} PanelToggle
 * @property {string} label
 * @property {string} help The line under it, or empty.
 * @property {"meters" | "notifications"} group
 * @property {string} key Its key inside the group.
 * @property {string | null} parent The key in the same group it is indented under and needs on.
 */

export const METERS_TITLE = "Meters";

export const METERS_HELP = "Rows shown in the popup.";

export const NOTIFICATIONS_TITLE = "Notifications";

/**
 * The meters; VRAM reads "GPU memory" where the GPU shares system memory.
 *
 * @param {boolean} sharesMemory
 * @returns {PanelToggle[]}
 */
export function meterToggles(sharesMemory) {
  return [
    toggle("CPU", "meters", "cpu"),
    toggle("RAM", "meters", "ram"),
    toggle("GPU", "meters", "gpu"),
    toggle(sharesMemory ? "GPU memory" : "VRAM", "meters", "vram"),
    toggle("Drives", "meters", "drives"),
    toggle("Drive activity", "meters", "drive_activity", "", "drives"),
  ];
}

/** @returns {PanelToggle[]} */
export function notificationToggles() {
  return [
    toggle(
      "Service stopped",
      "notifications",
      "service_stopped",
      "When the service stops without you stopping it.",
    ),
    toggle(
      "Running long",
      "notifications",
      "running_long",
      "When a job runs 1.5 times longer than expected.",
    ),
    toggle(
      "Benchmark finished",
      "notifications",
      "benchmark_finished",
      "When a benchmark you started ends.",
    ),
    toggle("Job finished", "notifications", "job_finished", "When any job ends."),
    toggle(
      "Killed outside the popup",
      "notifications",
      "killed_outside_flyout",
      "When a job is stopped by something other than this popup.",
    ),
  ];
}

/**
 * @param {TraySettings} settings
 * @param {PanelToggle} toggle
 * @returns {boolean}
 */
export function toggleValue(settings, toggle) {
  return groupOf(settings, toggle.group)[toggle.key] === true;
}

/**
 * Writes the toggle into `settings`, which the caller then saves.
 *
 * @param {TraySettings} settings
 * @param {PanelToggle} toggle
 * @param {boolean} on
 */
export function setToggle(settings, toggle, on) {
  groupOf(settings, toggle.group)[toggle.key] = on;
}

/**
 * Whether the switch can be changed: its parent, if any, is on.
 *
 * @param {TraySettings} settings
 * @param {PanelToggle} toggle
 * @returns {boolean}
 */
export function toggleEnabled(settings, toggle) {
  return toggle.parent === null || groupOf(settings, toggle.group)[toggle.parent] === true;
}

/**
 * One line per toggle that differs from its default, with the default, for Reset Page's
 * confirmation: "Job finished: Off".
 *
 * @param {TraySettings} settings
 * @param {boolean} sharesMemory
 * @returns {string[]}
 */
export function changedPanelLines(settings, sharesMemory) {
  const fresh = defaultSettings();
  return allToggles(sharesMemory)
    .filter((item) => toggleValue(settings, item) !== toggleValue(fresh, item))
    .map((item) => `${item.label}: ${onOff(toggleValue(fresh, item))}`);
}

/**
 * Puts every meter and notification toggle back to its default; the rest of `tray.json` stays.
 *
 * @param {TraySettings} settings
 */
export function resetPanel(settings) {
  const fresh = defaultSettings();
  for (const item of allToggles(false)) {
    setToggle(settings, item, toggleValue(fresh, item));
  }
}

/**
 * @param {boolean} sharesMemory
 * @returns {PanelToggle[]}
 */
function allToggles(sharesMemory) {
  return meterToggles(sharesMemory).concat(notificationToggles());
}

/**
 * @param {string} label
 * @param {"meters" | "notifications"} group
 * @param {string} key
 * @param {string} [help]
 * @param {string | null} [parent]
 * @returns {PanelToggle}
 */
function toggle(label, group, key, help = "", parent = null) {
  return { label, help, group, key, parent };
}

/**
 * @param {TraySettings} settings
 * @param {"meters" | "notifications"} group
 * @returns {Record<string, boolean>}
 */
function groupOf(settings, group) {
  return /** @type {Record<string, boolean>} */ (/** @type {unknown} */ (settings[group]));
}
