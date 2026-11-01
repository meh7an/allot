// Whether Plasma will show the widget's notifications. Nothing tells the widget when it drops them,
// so without these it would stay silent with no word why.

/** The name of the widget's notifyrc, and the component its notifications name. */
export const NOTIFY_COMPONENT = "allot";

/** The one event in the notifyrc: Allot's own toggles choose between its kinds. */
export const NOTIFY_EVENT = "notification";

/** The file in the package, and its name under `knotifications6` in the data folder. */
export const NOTIFY_RC_FILE = `${NOTIFY_COMPONENT}.notifyrc`;

export const INHIBITED_COMMAND =
  "gdbus call --session --dest org.freedesktop.Notifications " +
  "--object-path /org/freedesktop/Notifications " +
  "--method org.freedesktop.DBus.Properties.Get org.freedesktop.Notifications Inhibited";

/** The Notifications settings page, where a user turns Allot's notifications back on. */
export const SETTINGS_ARGV = ["systemsettings", "kcm_notifications"];

/**
 * @typedef {object} NotificationState
 * @property {boolean} doNotDisturb
 *   Plasma's Do Not Disturb is on: popups are hidden, but notifications still reach the history.
 * @property {boolean} off
 *   Allot's notifications are turned off in System Settings.
 */

/**
 * The answer of `INHIBITED_COMMAND`, which reads `(<true>,)`.
 *
 * @param {string} stdout
 * @returns {boolean}
 */
export function parseInhibited(stdout) {
  return stdout.includes("<true>");
}

/**
 * Whether System Settings turned Allot's notifications off. Its page keeps the app's switch in
 * `plasmanotifyrc` and the event's actions in the user's copy of the notifyrc.
 *
 * @param {string | null} eventConfig `~/.config/allot.notifyrc`
 * @param {string | null} plasmaConfig `~/.config/plasmanotifyrc`
 * @returns {boolean}
 */
export function isOffInSettings(eventConfig, plasmaConfig) {
  const eventAction = entry(eventConfig, `[Event/${NOTIFY_EVENT}]`, "Action");
  const eventOff = eventAction !== null && !eventAction.split("|").includes("Popup");
  const appPopups = entry(plasmaConfig, `[Services][${NOTIFY_COMPONENT}]`, "ShowPopups");
  return eventOff || appPopups === "false";
}

/**
 * A key's value in a KConfig file: the header is one line, as KConfig writes nested groups.
 *
 * @param {string | null} text
 * @param {string} group
 * @param {string} key
 * @returns {string | null}
 */
function entry(text, group, key) {
  if (text === null) {
    return null;
  }
  let inGroup = false;
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.startsWith("[")) {
      inGroup = line === group;
    } else if (inGroup && line.startsWith(`${key}=`)) {
      return line.slice(key.length + 1);
    }
  }
  return null;
}
