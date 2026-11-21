// The texts of a settings row that GNOME's GTK rows and the KDE widget's QML rows both build: the
// markup-safe string, the description a screen reader reads, and how a number shows. Each panel
// adds its own markup around them.

/**
 * Text for a label or row that must not be read as markup, by Pango or by Qt's rich text. The
 * apostrophe is a numeric reference, which both read.
 *
 * @param {string} text
 * @returns {string}
 */
export function markupEscaped(text) {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/**
 * What a screen reader says after a control's name: the help, the key, the notes, then the error
 * while one shows. It never holds a value that ticks, since Orca reads out each change.
 *
 * @param {string} help
 * @param {string | null} key
 * @param {Array<string | null>} [notes]
 * @param {string | null} [issue]
 * @returns {string}
 */
export function rowDescription(help, key, notes = [], issue = null) {
  const first = key === null || key === "" ? help : `${help} Key: ${key}.`;
  const lines = notes.filter((note) => note !== null && note !== "");
  const error = issue === null || issue === "" ? [] : [`Error: ${issue}`];
  return [first, ...lines, ...error].filter((part) => part !== "").join(" ");
}

/** The most decimals a number in a row shows. */
const MAX_DECIMALS = 2;

/**
 * A number rounded to at most two decimals and formatted by the toolkit's own locale code, with
 * no more decimals than it needs: 3 reads "3", 2.5 reads "2.5".
 *
 * @param {number} value
 * @param {(rounded: number, decimals: number) => string} format Formats `rounded` with exactly
 *   `decimals` decimals, in the user's locale.
 * @returns {string}
 */
export function formatRowNumber(value, format) {
  const scale = 10 ** MAX_DECIMALS;
  const rounded = Math.round(value * scale) / scale;
  let decimals = 0;
  while (decimals < MAX_DECIMALS && !Number.isInteger(rounded * 10 ** decimals)) {
    decimals += 1;
  }
  return format(rounded, decimals);
}
