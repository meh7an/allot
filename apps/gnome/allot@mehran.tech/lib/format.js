/** @import { ResourceUsage } from "../contract.g.mjs" */

// Text shared by several view models. Numbers never follow the user's locale: the UI is
// English, and a decimal comma would read as a list separator next to the handoff's middle dots.

const SHORT_ID_LENGTH = 8;

/**
 * The 8-character prefix the popup shows for a session UUID.
 *
 * @param {string} session
 * @returns {string}
 */
export function shortId(session) {
  return session.slice(0, SHORT_ID_LENGTH);
}

/**
 * The project folder: the last segment of a working directory, or empty without one. Both
 * separators count, since a state can come from a Windows daemon.
 *
 * @param {string | null} cwd
 * @returns {string}
 */
export function project(cwd) {
  const segments = (cwd ?? "").split(/[/\\]/).filter((segment) => segment !== "");
  return segments[segments.length - 1] ?? "";
}

/**
 * Live usage as `7.8 cores · 2.9 GB`.
 *
 * @param {ResourceUsage} usage
 * @returns {string}
 */
export function usage(usage) {
  return `${oneDecimal(usage.cores)} cores · ${gigabytes(usage.ram_mb)} GB`;
}

/**
 * Megabytes as gigabytes with one decimal, divided by 1024 as the other apps do.
 *
 * @param {number} megabytes
 * @returns {string}
 */
export function gigabytes(megabytes) {
  return oneDecimal(megabytes / 1024);
}

/**
 * Halves round away from zero, as the Windows app's .NET formatting does, so every app prints
 * the same figure for the same state.
 *
 * @param {number} value
 * @returns {string}
 */
function oneDecimal(value) {
  const rounded = (Math.sign(value) * Math.round(Math.abs(value) * 10)) / 10;
  return rounded.toFixed(1);
}
