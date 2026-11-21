import { NONE_FIT, fitsText } from "./live-text.js";
import { quantityText } from "./setting-values.js";

/**
 * @import { SizeFit, SizeRow } from "../contract.g.mjs"
 * @import { RowDraft, SizeRowView, SizeTableModel } from "./setting-values.js"
 * @import { Confirmation } from "./settings-text.js"
 */

// The Sizes page's sentences: table cells, the Fits now column, the edit dialog's title and live
// line, and the row actions' names, notices and confirmations.

/**
 * The cells of a class or preset row, each with its unit.
 *
 * @typedef {object} SizeCells
 * @property {string} cores "8".
 * @property {string} minCores "4", or "Automatic (4)" while the row names none.
 * @property {string} memory "4 GB".
 * @property {string} gpu "50 %".
 * @property {string} videoMemory "0 MB".
 * @property {string} runTime "5 minutes".
 */

/** The Fits now cell while no preview is known, such as while the service is stopped. */
export const FITS_UNKNOWN = "—";

export const ADD_PRESET_TITLE = "Add preset";

/**
 * The fewest cores a row that names no minimum starts with: half the cores, at least one, or
 * all of them when there are fewer (allotd's `default_min_cores`).
 *
 * @param {number} cores
 * @returns {number}
 */
export function automaticMinCores(cores) {
  return Math.max(cores / 2, Math.min(cores, 1));
}

/**
 * @param {SizeRow} row
 * @param {(value: number) => string} number
 * @returns {SizeCells}
 */
export function sizeCells(row, number) {
  const profile = row.profile;
  return {
    cores: number(profile.cores),
    minCores:
      row.min_cores === null
        ? `Automatic (${number(automaticMinCores(profile.cores))})`
        : number(row.min_cores),
    memory: quantityText(profile.ram_mb, "megabytes", number),
    gpu: quantityText(profile.gpu_pct, "percent", number),
    videoMemory: quantityText(profile.vram_mb, "megabytes", number),
    runTime: quantityText(profile.est_secs, "seconds", number),
  };
}

/**
 * A row as one line, for the subtitle of a row that stands in for a table row on GNOME and KDE:
 * "8 cores, at least 4 · 4 GB · GPU 0 % · 5 minutes · fits 1, 2 more shrunk".
 *
 * @param {SizeRow} row
 * @param {SizeFit | null} fit The row's fit preview; the fits part is left out without one.
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function sizeSummary(row, fit, number) {
  const cells = sizeCells(row, number);
  const minimum =
    row.min_cores === null
      ? `at least ${number(automaticMinCores(row.profile.cores))} (automatic)`
      : `at least ${cells.minCores}`;
  const parts = [
    `${quantityText(row.profile.cores, "cores", number)}, ${minimum}`,
    cells.memory,
    `GPU ${cells.gpu}`,
  ];
  if (row.profile.vram_mb > 0) {
    parts.push(`${cells.videoMemory} video memory`);
  }
  parts.push(cells.runTime);
  if (fit !== null) {
    parts.push(`fits ${fitsNowText(fit, number).toLowerCase()}`);
  }
  return parts.join(" · ");
}

/**
 * The Fits now cell: "1, 2 more shrunk", "2 shrunk", "3", "at least 99" or "None".
 *
 * @param {SizeFit | null} fit The row's fit preview, or `null` while none is known.
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function fitsNowText(fit, number) {
  if (fit === null) {
    return FITS_UNKNOWN;
  }
  const fits = fit.now;
  if (fits.full === 0 && fits.shrunk === 0) {
    return "None";
  }
  // Counts stop at allotd's limit; the larger count then reads "at least 99".
  const capped = fits.more ? "at least " : "";
  if (fits.shrunk === 0) {
    return `${capped}${number(fits.full)}`;
  }
  const shrunk = `${capped}${number(fits.shrunk)}`;
  return fits.full === 0 ? `${shrunk} shrunk` : `${number(fits.full)}, ${shrunk} more shrunk`;
}

/**
 * The edit dialog's live line: "With these values, now: 1 at full size, 2 more at 4 cores. With
 * every lease ended: 2 at full size."
 *
 * @param {SizeFit} fit
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function dialogFitsText(fit, number) {
  const now = lowered(fitsText(fit.now, fit.min_cores, number));
  const idle = lowered(fitsText(fit.idle, fit.min_cores, number));
  return `With these values, now: ${now}. With every lease ended: ${idle}.`;
}

/**
 * The edit dialog's title: "Edit CPU heavy", "Edit preset cpu_small" or "Add preset".
 *
 * @param {SizeTableModel} table
 * @param {RowDraft} draft
 * @returns {string}
 */
export function sizeDialogTitle(table, draft) {
  const name = draft.original;
  if (name === null) {
    return ADD_PRESET_TITLE;
  }
  if (table.canAdd) {
    return `Edit preset ${name}`;
  }
  return `Edit ${table.rows.find((view) => view.row.name === name)?.label ?? name}`;
}

/**
 * The dialog's warning while a preset is renamed; `null` otherwise.
 *
 * @param {RowDraft} draft
 * @returns {string | null}
 */
export function renameWarning(draft) {
  const original = draft.original;
  if (original === null || !draft.canRename || draft.name === "" || draft.name === original) {
    return null;
  }
  return `Sessions that ask for ${original} get an error until they use ${draft.name}.`;
}

/**
 * The accessible name of a row's Edit button: "Edit CPU heavy", "Edit preset cpu_small".
 *
 * @param {SizeTableModel} table
 * @param {SizeRowView} view
 * @returns {string}
 */
export function editRowName(table, view) {
  return `Edit ${subject(table, view)}`;
}

/**
 * The accessible name of a row's reset button: "Reset CPU heavy to its built-in size".
 *
 * @param {SizeTableModel} table
 * @param {SizeRowView} view
 * @returns {string}
 */
export function resetRowName(table, view) {
  return `Reset ${subject(table, view)} to its built-in size`;
}

/**
 * The accessible name of a preset's delete button: "Delete preset render".
 *
 * @param {SizeRowView} view
 * @returns {string}
 */
export function deleteRowName(view) {
  return `Delete preset ${view.row.name}`;
}

/**
 * The notice after a row's reset or a preset's delete, both with Undo.
 *
 * @param {SizeTableModel} table
 * @param {SizeRowView} view
 * @returns {string}
 */
export function rowRemovedText(table, view) {
  return view.builtIn
    ? `${capitalized(subject(table, view))} reset to its built-in size.`
    : `Preset ${view.row.name} deleted.`;
}

/**
 * @param {string} name
 * @returns {Confirmation}
 */
export function deletePresetConfirmation(name) {
  return {
    title: `Delete preset ${name}?`,
    body: "Sessions that ask for it by name get an error. You can add it again later.",
    button: "Delete Preset",
  };
}

/**
 * "CPU heavy" for a class, "preset cpu_small" for a preset.
 *
 * @param {SizeTableModel} table
 * @param {SizeRowView} view
 * @returns {string}
 */
function subject(table, view) {
  return table.canAdd ? `preset ${view.row.name}` : view.label;
}

/**
 * @param {string} text
 * @returns {string}
 */
function capitalized(text) {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/**
 * "None fit" reads lowercase inside a sentence.
 *
 * @param {string} fits
 * @returns {string}
 */
function lowered(fits) {
  return fits === NONE_FIT ? "none fit" : fits;
}
