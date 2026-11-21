// Generated from apps/gnome/allot@mehran.tech/lib/live-text.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
import { quantityText } from "./setting-values.js";
import { joined } from "./settings-text.js";
/** @import { Fits, KeptFree } from "../contract.g.mjs" */
// The live sentence at the end of each group with numbers, built from the values
// on screen or from allotd's fit preview.
export const SERVICE_STOPPED_TEXT = "Can't work this out while the service is stopped.";
export const LOWERING_OFF_TEXT = "Requests are reserved as declared, or higher when past runs used more.";
export const NONE_FIT = "None fit";
// The example in the learned-estimate sentence: a command asking for 8 cores that used 3.
const EXAMPLE_ASKED = 8;
const EXAMPLE_USED = 3;
/**
 * "Keeps 2.4 of 16 cores, 4.8 of 32 GB memory, 15 % of the GPU and 1.2 of 8 GB video memory
 * free." The GPU parts are left out without a dedicated adapter.
 *
 * @param {KeptFree} kept
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function headroomText(kept, number) {
    const parts = [
        `${number(kept.cores)} of ${number(kept.total_cores)} cores`,
        `${gigabytes(kept.ram_mb, number)} of ${gigabytes(kept.total_ram_mb, number)} GB memory`,
    ];
    if (kept.gpu_pct !== null) {
        parts.push(`${number(kept.gpu_pct)} % of the GPU`);
    }
    if (kept.vram_mb !== null && kept.total_vram_mb !== null) {
        parts.push(`${gigabytes(kept.vram_mb, number)} of ${gigabytes(kept.total_vram_mb, number)} GB ` +
            "video memory");
    }
    return `Keeps ${joined(parts)} free.`;
}
/**
 * What learning from past runs does to an example command, with the values on screen.
 *
 * @param {{ lower: boolean, minRuns: number, marginPct: number }} values
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function estimatesText(values, number) {
    if (!values.lower) {
        return LOWERING_OFF_TEXT;
    }
    const reserved = Math.round(EXAMPLE_USED * (1 + values.marginPct / 100) * 100) / 100;
    const runs = values.minRuns === 1 ? "run" : `${number(values.minRuns)} runs`;
    return (`A command that asks for ${number(EXAMPLE_ASKED)} cores but used about ` +
        `${number(EXAMPLE_USED)} in its last ${runs} is reserved ` +
        `${quantityText(reserved, "cores", number)}. Requests never go below 0.25 cores, 256 MB, ` +
        "or their minimum cores, and run time never lowers.");
}
/**
 * A cell of What fits: "1 at full size, 2 more at 4 cores", "at least 99 at full size", or
 * "None fit".
 *
 * @param {Fits} fits
 * @param {number} minCores
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function fitsText(fits, minCores, number) {
    if (fits.full === 0 && fits.shrunk === 0) {
        return NONE_FIT;
    }
    // Counts stop at allotd's limit; the larger count then reads "at least 99".
    const capped = fits.more ? "at least " : "";
    const parts = [];
    if (fits.full > 0) {
        const full = fits.shrunk === 0 ? capped : "";
        parts.push(`${full}${number(fits.full)} at full size`);
    }
    if (fits.shrunk > 0) {
        const more = fits.full > 0 ? " more" : "";
        parts.push(`${capped}${number(fits.shrunk)}${more} at ${quantityText(minCores, "cores", number)}`);
    }
    return parts.join(", ");
}
/**
 * The caption under What fits.
 *
 * @param {number} cpuBusyPct
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function fitsCaption(cpuBusyPct, number) {
    return (`Based on this machine now (${number(Math.round(cpuBusyPct))} % CPU busy) and with every ` +
        "lease ended.");
}
/**
 * "A benchmark starts once CPU and GPU stay at or below 10 % for 10 seconds. If that takes
 * longer than 1 minute, it starts anyway, marked not quiet."
 *
 * @param {{ cpuPct: number, gpuPct: number, quietSecs: number, maxWaitSecs: number }} values
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function benchmarksText(values, number) {
    const cpu = quantityText(values.cpuPct, "percent", number);
    const gpu = quantityText(values.gpuPct, "percent", number);
    const loads = values.cpuPct === values.gpuPct
        ? `CPU and GPU stay at or below ${cpu}`
        : `CPU stays at or below ${cpu} and GPU at or below ${gpu}`;
    const quiet = quantityText(values.quietSecs, "seconds", number);
    const wait = quantityText(values.maxWaitSecs, "seconds", number);
    return (`A benchmark starts once ${loads} for ${quiet}. If that takes longer than ${wait}, it ` +
        "starts anyway, marked not quiet.");
}
/**
 * "Keeps ended leases for 30 days and job runs for 90 days. Each command keeps at least its
 * newest 20 runs, and never more than 5,000 runs are stored."
 *
 * @param {{ leaseDays: number, historyDays: number, keepRuns: number, maxRuns: number }} values
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function historyText(values, number) {
    const newest = values.keepRuns === 1 ? "newest run" : `newest ${number(values.keepRuns)} runs`;
    const most = values.maxRuns === 1 ? "1 run is" : `${number(values.maxRuns)} runs are`;
    const leases = quantityText(values.leaseDays, "days", number);
    const runs = quantityText(values.historyDays, "days", number);
    return (`Keeps ended leases for ${leases} and job runs for ${runs}. Each command keeps at least ` +
        `its ${newest}, and never more than ${most} stored.`);
}
/**
 * @param {number} megabytes
 * @param {(value: number) => string} number
 * @returns {string}
 */
function gigabytes(megabytes, number) {
    return number(Math.round((megabytes / 1024) * 10) / 10);
}
