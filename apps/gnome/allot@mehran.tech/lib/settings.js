/**
 * The More options > Meters check items; all on by default.
 *
 * @typedef {object} MeterToggles
 * @property {boolean} cpu
 * @property {boolean} ram
 * @property {boolean} gpu
 * @property {boolean} vram
 * @property {boolean} drives
 * @property {boolean} drive_activity
 *   Read and write rates on the drive rows; disabled in the menu while Drives is off.
 */

/**
 * The More options > Notifications check items. The last key keeps its Windows name, since every
 * app reads the same file.
 *
 * @typedef {object} NotificationToggles
 * @property {boolean} service_stopped
 * @property {boolean} running_long
 * @property {boolean} benchmark_finished
 * @property {boolean} job_finished
 * @property {boolean} killed_outside_flyout
 */

/**
 * What the popup remembers in `tray.json`, in the other apps' format.
 *
 * @typedef {object} TraySettings
 * @property {MeterToggles} meters
 * @property {NotificationToggles} notifications
 * @property {boolean} resources_expanded
 *   Whether the Resources card shows its rows; collapsed shows the summary line.
 * @property {boolean} first_run_done
 *   The first-run card was dismissed with Done.
 * @property {boolean} first_run_shown
 *   The first-run card has shown once: the popup opened by itself, which it does not do again.
 */

/** @returns {TraySettings} */
export function defaultSettings() {
  return {
    meters: { cpu: true, ram: true, gpu: true, vram: true, drives: true, drive_activity: true },
    notifications: {
      service_stopped: true,
      running_long: true,
      benchmark_finished: true,
      job_finished: false,
      killed_outside_flyout: false,
    },
    resources_expanded: false,
    first_run_done: false,
    first_run_shown: false,
  };
}

/**
 * Reads `tray.json`. Keys the file lacks keep their defaults and unknown ones are ignored; a
 * missing or unreadable file gives the defaults, and the next save replaces it.
 *
 * @param {string | null} text
 * @returns {TraySettings}
 */
export function decodeSettings(text) {
  const settings = defaultSettings();
  if (text === null) {
    return settings;
  }
  let read;
  try {
    read = JSON.parse(text);
  } catch {
    return settings;
  }
  if (!isObject(read)) {
    return settings;
  }
  overlay(settings.meters, read.meters);
  overlay(settings.notifications, read.notifications);
  overlay(settings, read);
  return settings;
}

/**
 * @param {TraySettings} settings
 * @returns {string} Indented JSON, as the other apps write it.
 */
export function encodeSettings(settings) {
  return `${JSON.stringify(settings, null, 2)}\n`;
}

/**
 * Copies the booleans `source` has over the same keys of `target`.
 *
 * @param {object} target
 * @param {unknown} source
 */
function overlay(target, source) {
  if (!isObject(source)) {
    return;
  }
  const values = /** @type {Record<string, unknown>} */ (target);
  for (const key of Object.keys(values)) {
    if (typeof values[key] === "boolean" && typeof source[key] === "boolean") {
      values[key] = source[key];
    }
  }
}

/**
 * @param {unknown} value
 * @returns {value is Record<string, unknown>}
 */
function isObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
