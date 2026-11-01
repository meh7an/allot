// Generated from apps/gnome/allot@mehran.tech/lib/format.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/** @import { JobProfile, ResourceUsage } from "../contract.g.mjs" */
// Text shared by several view models. Numbers never follow the user's locale: the UI is
// English, and a decimal comma would read as a list separator next to the handoff's middle dots.
const SHORT_ID_LENGTH = 8;
const INITIALS_LENGTH = 2;
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
    var _a;
    const segments = (cwd !== null && cwd !== void 0 ? cwd : "").split(/[/\\]/).filter((segment) => segment !== "");
    return (_a = segments[segments.length - 1]) !== null && _a !== void 0 ? _a : "";
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
 * A reservation as `8 cores · 4 GB` or `2 cores · 4 GB · GPU 80% · 4 GB VRAM`. Cores always
 * show; RAM, GPU and VRAM only when reserved.
 *
 * @param {JobProfile} profile
 * @returns {string}
 */
export function reservation(profile) {
    const parts = [cores(profile.cores)];
    if (profile.ram_mb > 0) {
        parts.push(`${size(profile.ram_mb)} GB`);
    }
    if (profile.gpu_pct > 0) {
        parts.push(`GPU ${whole(profile.gpu_pct)}%`);
    }
    if (profile.vram_mb > 0) {
        parts.push(`${size(profile.vram_mb)} GB VRAM`);
    }
    return parts.join(" · ");
}
/**
 * `1 core`, `8 cores`, `2.5 cores`.
 *
 * @param {number} count
 * @returns {string}
 */
export function cores(count) {
    const number = upToOneDecimal(count);
    return number === "1" ? "1 core" : `${number} cores`;
}
/**
 * A duration as `3:06`, or `1:02:03` from one hour.
 *
 * @param {number} seconds Whole seconds.
 * @returns {string}
 */
export function duration(seconds) {
    const total = Math.max(0, Math.floor(seconds));
    const hours = Math.floor(total / 3600);
    const rest = total % 60;
    return hours > 0
        ? `${hours}:${twoDigits(Math.floor(total / 60) % 60)}:${twoDigits(rest)}`
        : `${Math.floor(total / 60)}:${twoDigits(rest)}`;
}
/**
 * A rough wait as `2 min` or `1 h 20 min`, rounded to whole minutes and never less than one.
 *
 * @param {number} seconds
 * @returns {string}
 */
export function minutes(seconds) {
    const total = Math.max(1, roundAway(seconds / 60));
    if (total < 60) {
        return `${total} min`;
    }
    const hours = Math.floor(total / 60);
    const rest = total % 60;
    return rest === 0 ? `${hours} h` : `${hours} h ${rest} min`;
}
/**
 * How long ago something ended: `just now`, `4 min ago`, `2 h ago`, `3 d ago`.
 *
 * @param {number} seconds Whole seconds.
 * @returns {string}
 */
export function ago(seconds) {
    if (seconds < 60) {
        return "just now";
    }
    if (seconds < 3600) {
        return `${Math.floor(seconds / 60)} min ago`;
    }
    if (seconds < 86400) {
        return `${Math.floor(seconds / 3600)} h ago`;
    }
    return `${Math.floor(seconds / 86400)} d ago`;
}
/**
 * The two-letter placeholder for a process tile: `cargo` gives `CA`. Paths, quotes and a
 * Windows `.exe` suffix are ignored; empty when nothing is left.
 *
 * @param {string} program
 * @returns {string}
 */
export function initials(program) {
    let name = program.trim().replace(/^"+/, "").replace(/"+$/, "");
    name = name.slice(Math.max(name.lastIndexOf("/"), name.lastIndexOf("\\")) + 1);
    if (name.toLowerCase().endsWith(".exe")) {
        name = name.slice(0, -4);
    }
    let letters = "";
    for (const character of name) {
        if (letters.length < INITIALS_LENGTH && isLetterOrDigit(character)) {
            letters += character;
        }
    }
    return letters.toUpperCase();
}
/**
 * Megabytes as gigabytes, whole ones without decimals and others with one: `4`, `1.5`.
 *
 * @param {number} megabytes
 * @returns {string}
 */
export function size(megabytes) {
    return upToOneDecimal(megabytes / 1024);
}
/**
 * A whole number, as `GPU 80%` and `3% CPU` print it.
 *
 * @param {number} value
 * @returns {string}
 */
export function whole(value) {
    return roundAway(value).toFixed(0);
}
/**
 * One decimal. Halves round away from zero, as the Windows app's .NET formatting does, so
 * every app prints the same figure for the same state.
 *
 * @param {number} value
 * @returns {string}
 */
export function oneDecimal(value) {
    return (roundAway(value * 10) / 10).toFixed(1);
}
/**
 * .NET's `0.#`: one decimal, dropped when it is zero.
 *
 * @param {number} value
 * @returns {string}
 */
function upToOneDecimal(value) {
    const rounded = roundAway(value * 10) / 10;
    return rounded.toFixed(Number.isInteger(rounded) ? 0 : 1);
}
/**
 * @param {number} value
 * @returns {number}
 */
function roundAway(value) {
    return Math.sign(value) * Math.round(Math.abs(value));
}
/**
 * @param {number} value
 * @returns {string}
 */
function twoDigits(value) {
    return value < 10 ? `0${value}` : `${value}`;
}
/**
 * QML's JavaScript has no Unicode property escapes, so a letter is any character with a case,
 * which covers the scripts process names use.
 *
 * @param {string} character
 * @returns {boolean}
 */
function isLetterOrDigit(character) {
    return ((character >= "0" && character <= "9") || character.toLowerCase() !== character.toUpperCase());
}
