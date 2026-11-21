import { issueText } from "./setting-values.js";

/**
 * @import { SettingsBanner } from "./settings-banners.js"
 * @import { SaveStatus, SettingsSession } from "./settings-session.js"
 */

// What a Settings window says to a screen reader by itself, without a focus change (Mehran at
// M7.1: errors and outcomes): a field's new error, a banner that appears, and the outcome of a
// save. Live sentences and What fits are not spoken; they are read with their group when focus
// reaches it. Each panel speaks the text through its own toolkit.

export const SAVED_ANNOUNCEMENT = "All changes saved";

/**
 * What the announcer compares between two changes of the window.
 *
 * @typedef {object} AnnouncerView
 * @property {string[]} banners The shown page's banner messages.
 * @property {Array<{ key: string, text: string }>} issues Each setting with an error, with the
 *   sentence to speak: "Headroom: Enter a number at least 0 and below 100."
 * @property {SaveStatus["kind"]} save
 */

/**
 * The announcer's view of a session and the banners of the page shown.
 *
 * @param {SettingsSession} session
 * @param {SettingsBanner[]} banners
 * @param {(value: number) => string} number Formats a number for the user's locale.
 * @returns {AnnouncerView}
 */
export function announcerView(session, banners, number) {
  /** @type {Array<{ key: string, text: string }>} */
  const issues = [];
  for (const page of session.pages) {
    for (const group of page.groups) {
      for (const setting of group.settings) {
        if (setting.issue !== null) {
          issues.push({
            key: setting.key,
            text: `${setting.label}: ${issueText(setting.issue, number)}`,
          });
        }
      }
    }
  }
  return {
    banners: banners.map((banner) => banner.message),
    issues,
    save: session.saveStatus.kind,
  };
}

/**
 * Turns successive views into what to speak: each new or changed error once, each banner as it
 * appears, and "All changes saved" when a save ends with nothing left unsaved. The first view,
 * and the first after another page shows, only sets what is already on screen.
 */
export class SettingsAnnouncer {
  /** @type {Set<string> | null} */
  #banners = null;
  /** @type {Map<string, string>} */
  #issues = new Map();
  #saving = false;

  /**
   * What to speak for this view, as one text; empty when nothing changed that is spoken.
   *
   * @param {AnnouncerView} view
   * @returns {string}
   */
  update(view) {
    /** @type {string[]} */
    const spoken = [];
    if (this.#banners !== null) {
      spoken.push(...view.banners.filter((message) => !this.#banners?.has(message)));
    }
    for (const issue of view.issues) {
      if (this.#issues.get(issue.key) !== issue.text) {
        spoken.push(issue.text);
      }
    }
    if (this.#saving && view.save === "saved" && spoken.length === 0) {
      spoken.push(SAVED_ANNOUNCEMENT);
    }
    this.#banners = new Set(view.banners);
    this.#issues = new Map(view.issues.map((issue) => [issue.key, issue.text]));
    this.#saving = view.save === "saving";
    return spoken.join(" ");
  }

  /** Another page shows: its banners are read with the page, not announced. */
  pageShown() {
    this.#banners = null;
  }
}
