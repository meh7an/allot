// Generated from apps/gnome/allot@mehran.tech/lib/resources-view.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
import * as format from "./format.js";
/** @import { CpuMeter, DriveUsage, GpuMeter } from "../contract.g.mjs" */
/** @import { Meters, RamMeter, UiState } from "../contract.g.mjs" */
/** @import { ServiceStatus } from "./client.js" */
/** @import { MeterToggles } from "./settings.js" */
/**
 * One bullet-bar row of the Resources card: CPU, RAM, or a GPU's engine load or video memory.
 *
 * @typedef {object} MeterRow
 * @property {string} id
 *   Stable across polls: `cpu`, `ram`, `gpu:0`, `vram:0`.
 * @property {string} label
 *   For example `CPU`, or `GPU 0` with several adapters.
 * @property {number} used
 *   The used fill, as a fraction of the bar.
 * @property {number | null} shared
 *   The part of the used fill that is GPU memory, as a fraction of the bar; only on the RAM row
 *   of a machine whose GPU shares RAM.
 * @property {number | null} reserved
 *   Where the reserved tick sits, as a fraction of the bar; `null` without a reservation.
 * @property {number} headroom
 *   Where the hatched headroom starts, as a fraction of the bar.
 * @property {string} value
 *   For example `64%` or `22.4 / 32 GB`.
 * @property {string} detail
 *   The row's hover label, for example `10 of 16 cores reserved`.
 * @property {number[]} spark
 *   The CPU sparkline's samples as fractions, oldest first; empty on other rows.
 */
/**
 * One drive of the Resources card.
 *
 * @typedef {object} DriveRow
 * @property {string} id
 *   The mount point allotd names the drive by, which is also its hover label.
 * @property {string} label
 *   The mount point's last part, or `/`.
 * @property {number} used
 *   The usage bar's fill, as a fraction.
 * @property {string} activity
 *   For example `R 42 · W 8 MB/s` or `Idle`; empty while Drive activity is off.
 * @property {string} value
 *   For example `612 / 953 GB` or `1.21 / 1.82 TB`.
 */
/**
 * The docked Resources card for one poll (handoff section 5). The Windows tray app's
 * `ResourcesView`, except that a GPU sharing RAM has no VRAM row: its memory shows as a tint
 * inside the RAM bar.
 *
 * @typedef {object} ResourcesView
 * @property {boolean} isVisible
 *   The service serves live state; there is no card without data.
 * @property {boolean} isExpanded
 *   The card shows its rows instead of the summary.
 * @property {string} summary
 *   The line shown while collapsed, for example `CPU 64% · RAM 22.4 / 32 GB · GPU 78%`; empty
 *   while expanded.
 * @property {MeterRow[]} meters
 *   CPU, RAM, then GPU and VRAM per adapter, as the toggles allow.
 * @property {DriveRow[]} drives
 *   One row per drive while Drives is on.
 * @property {boolean} showsSeparator
 *   The hairline between the meters and the drives.
 * @property {boolean} allOff
 *   Every meter is off; the card says how to turn them on.
 * @property {boolean} showsLegend
 *   Some bar has a reserved tick, so the legend explains it.
 * @property {boolean} showsSharedLegend
 *   The RAM bar has a GPU tint, so the legend names it.
 * @property {boolean} sharesMemory
 *   Some GPU uses RAM as its memory, so the Meters menu calls VRAM `GPU Memory`.
 */
/**
 * What the leases reserve as a whole, for the rows' details.
 *
 * @typedef {object} Reservations
 * @property {boolean} benchmark
 *   A benchmark running alone has the whole machine.
 * @property {string} none
 *   A bar without a reservation: `No reservations` when nothing at all is reserved, else
 *   `Not reserved`.
 */
const SPARK_SAMPLES = 24;
const MEBIBYTES_PER_TEBIBYTE = 1024 * 1024;
const BYTES_PER_MEBIBYTE = 1024 * 1024;
const IDLE_RATE_MB = 0.1;
const WHOLE_RATE_MB = 10;
const WHOLE_MACHINE = "Whole machine reserved";
/** @type {ResourcesView} */
const HIDDEN = {
    isVisible: false,
    isExpanded: false,
    summary: "",
    meters: [],
    drives: [],
    showsSeparator: false,
    allOff: false,
    showsLegend: false,
    showsSharedLegend: false,
    sharesMemory: false,
};
/**
 * @param {ServiceStatus} status The service status of the latest poll.
 * @param {UiState | null} state The latest state; ignored unless the service is running.
 * @param {MeterToggles} toggles
 * @param {boolean} expanded The card shows its rows instead of the summary.
 * @returns {ResourcesView}
 */
export function resourcesView(status, state, toggles, expanded) {
    if (status !== "running" || state === null) {
        return HIDDEN;
    }
    const { meters } = state;
    const reservations = reservationsOf(state);
    const headroom = clamp(1 - meters.headroom_pct / 100);
    // allotd counts a shared adapter's memory in RAM already, and admission adds its reserved
    // video memory to the reserved RAM.
    const sharing = meters.gpus.filter((gpu) => gpu.unified_memory);
    const sharedMb = toggles.vram ? sum(sharing.map((gpu) => gpu.vram_used_mb)) : 0;
    const sharedReservedMb = sum(sharing.map((gpu) => gpu.reserved_vram_mb));
    /** @type {MeterRow[]} */
    const rows = [];
    if (toggles.cpu) {
        rows.push(cpuRow(meters.cpu, reservations, headroom));
    }
    if (toggles.ram) {
        rows.push(ramRow(meters.ram, sharedMb, sharedReservedMb, reservations, headroom));
    }
    const several = meters.gpus.length > 1;
    meters.gpus.forEach((gpu, index) => {
        const suffix = several ? ` ${index}` : "";
        const prefix = several ? `${gpu.name} · ` : "";
        if (toggles.gpu) {
            rows.push(gpuRow(gpu, `gpu:${index}`, `GPU${suffix}`, prefix, reservations, headroom));
        }
        if (toggles.vram && !gpu.unified_memory) {
            rows.push(vramRow(gpu, `vram:${index}`, `VRAM${suffix}`, prefix, reservations, headroom));
        }
    });
    const drives = toggles.drives
        ? meters.drives.map((drive) => driveRow(drive, toggles.drive_activity))
        : [];
    const allOn = [toggles.cpu, toggles.ram, toggles.gpu, toggles.vram, toggles.drives];
    return {
        isVisible: true,
        isExpanded: expanded,
        summary: expanded ? "" : summaryLine(meters, toggles),
        meters: rows,
        drives,
        showsSeparator: rows.length > 0 && drives.length > 0,
        allOff: !allOn.some((on) => on),
        showsLegend: rows.some((row) => row.reserved !== null),
        showsSharedLegend: rows.some((row) => row.shared !== null),
        sharesMemory: sharing.length > 0,
    };
}
/**
 * A drive's label: the mount point's last part, or the mount point itself for `/`.
 *
 * @param {string} mountPoint
 * @returns {string}
 */
export function driveLabel(mountPoint) {
    return format.project(mountPoint) || mountPoint;
}
/**
 * @param {CpuMeter} cpu
 * @param {Reservations} reservations
 * @param {number} headroom
 * @returns {MeterRow}
 */
function cpuRow(cpu, reservations, headroom) {
    const cores = format.upToOneDecimal(cpu.reserved_cores);
    let detail = reservations.none;
    if (reservations.benchmark) {
        detail = "Whole machine reserved for the benchmark";
    }
    else if (cpu.reserved_cores > 0) {
        detail = `${cores} of ${cpu.logical_cores} cores reserved`;
    }
    const reserved = reservations.benchmark ? 1 : fraction(cpu.reserved_cores, cpu.logical_cores);
    return {
        id: "cpu",
        label: "CPU",
        used: clamp(cpu.usage_pct / 100),
        shared: null,
        reserved: tick(reserved),
        headroom,
        value: percent(cpu.usage_pct),
        detail,
        spark: cpu.history_pct.slice(-SPARK_SAMPLES).map((pct) => clamp(pct / 100)),
    };
}
/**
 * @param {RamMeter} ram
 * @param {number} sharedMb
 * @param {number} sharedReservedMb
 * @param {Reservations} reservations
 * @param {number} headroom
 * @returns {MeterRow}
 */
function ramRow(ram, sharedMb, sharedReservedMb, reservations, headroom) {
    const reservedMb = ram.reserved_mb + sharedReservedMb;
    let detail = reservations.none;
    if (reservations.benchmark) {
        detail = WHOLE_MACHINE;
    }
    else if (reservedMb > 0) {
        detail = `${format.size(reservedMb)} GB reserved`;
    }
    if (sharedMb > 0) {
        detail += ` · GPU uses ${format.gigabytes(sharedMb)} GB of it`;
    }
    const used = fraction(ram.used_mb, ram.total_mb);
    return {
        id: "ram",
        label: "RAM",
        used,
        shared: sharedMb > 0 ? Math.min(used, fraction(sharedMb, ram.total_mb)) : null,
        reserved: tick(reservations.benchmark ? 1 : fraction(reservedMb, ram.total_mb)),
        headroom,
        value: `${format.gigabytes(ram.used_mb)} / ${format.size(ram.total_mb)} GB`,
        detail,
        spark: [],
    };
}
/**
 * @param {GpuMeter} gpu
 * @param {string} id
 * @param {string} label
 * @param {string} prefix
 * @param {Reservations} reservations
 * @param {number} headroom
 * @returns {MeterRow}
 */
function gpuRow(gpu, id, label, prefix, reservations, headroom) {
    let detail = reservations.none;
    if (reservations.benchmark) {
        detail = WHOLE_MACHINE;
    }
    else if (gpu.reserved_pct > 0) {
        detail = `${percent(gpu.reserved_pct)} reserved`;
    }
    return {
        id,
        label,
        used: clamp(gpu.busiest_pct / 100),
        shared: null,
        reserved: tick(reservations.benchmark ? 1 : clamp(gpu.reserved_pct / 100)),
        headroom,
        value: percent(gpu.busiest_pct),
        detail: prefix + detail,
        spark: [],
    };
}
/**
 * Without a reported total there is nothing to measure the bar against: it stays empty, and the
 * value reads the used memory alone.
 *
 * @param {GpuMeter} gpu
 * @param {string} id
 * @param {string} label
 * @param {string} prefix
 * @param {Reservations} reservations
 * @param {number} headroom
 * @returns {MeterRow}
 */
function vramRow(gpu, id, label, prefix, reservations, headroom) {
    let detail = reservations.none;
    if (reservations.benchmark) {
        detail = WHOLE_MACHINE;
    }
    else if (gpu.reserved_vram_mb > 0) {
        detail = `${format.size(gpu.reserved_vram_mb)} GB reserved`;
    }
    const total = gpu.vram_total_mb;
    if (total === null) {
        return {
            id,
            label,
            used: 0,
            shared: null,
            reserved: null,
            headroom,
            value: `${format.gigabytes(gpu.vram_used_mb)} GB`,
            detail: `${prefix}${detail} · total not reported`,
            spark: [],
        };
    }
    return {
        id,
        label,
        used: fraction(gpu.vram_used_mb, total),
        shared: null,
        reserved: tick(reservations.benchmark ? 1 : fraction(gpu.reserved_vram_mb, total)),
        headroom,
        value: `${format.gigabytes(gpu.vram_used_mb)} / ${format.size(total)} GB`,
        detail: prefix + detail,
        spark: [],
    };
}
/**
 * @param {DriveUsage} drive
 * @param {boolean} activity
 * @returns {DriveRow}
 */
function driveRow(drive, activity) {
    return {
        id: drive.name,
        label: driveLabel(drive.name),
        used: fraction(drive.used_mb, drive.total_mb),
        activity: activity ? activityLine(drive) : "",
        value: space(drive.used_mb, drive.total_mb),
    };
}
/**
 * `612 / 953 GB` in whole gigabytes, or from 1000 GB `1.21 / 1.82 TB`.
 *
 * @param {number} usedMb
 * @param {number} totalMb
 * @returns {string}
 */
function space(usedMb, totalMb) {
    const totalGb = totalMb / 1024;
    if (Math.round(totalGb) < 1000) {
        return `${format.whole(usedMb / 1024)} / ${format.whole(totalGb)} GB`;
    }
    const used = format.upToTwoDecimals(usedMb / MEBIBYTES_PER_TEBIBYTE);
    const total = format.upToTwoDecimals(totalMb / MEBIBYTES_PER_TEBIBYTE);
    return `${used} / ${total} TB`;
}
/**
 * `R 42 · W 8 MB/s`, one side alone when the other is idle, or `Idle`.
 *
 * @param {DriveUsage} drive
 * @returns {string}
 */
function activityLine(drive) {
    const read = rate(drive.read_bytes_per_sec);
    const write = rate(drive.write_bytes_per_sec);
    const parts = [];
    if (read !== null) {
        parts.push(`R ${read}`);
    }
    if (write !== null) {
        parts.push(`W ${write}`);
    }
    return parts.length === 0 ? "Idle" : `${parts.join(" · ")} MB/s`;
}
/**
 * Megabytes per second with one decimal below 10, whole above; `null` under 0.1 MB/s.
 *
 * @param {number} bytesPerSec
 * @returns {string | null}
 */
function rate(bytesPerSec) {
    const megabytes = bytesPerSec / BYTES_PER_MEBIBYTE;
    if (megabytes < IDLE_RATE_MB) {
        return null;
    }
    return megabytes < WHOLE_RATE_MB ? format.upToOneDecimal(megabytes) : format.whole(megabytes);
}
/**
 * The first adapter stands for the GPU in the summary, as on the handoff's board.
 *
 * @param {Meters} meters
 * @param {MeterToggles} toggles
 * @returns {string}
 */
function summaryLine(meters, toggles) {
    const parts = [];
    if (toggles.cpu) {
        parts.push(`CPU ${percent(meters.cpu.usage_pct)}`);
    }
    if (toggles.ram) {
        const { used_mb: used, total_mb: total } = meters.ram;
        parts.push(`RAM ${format.gigabytes(used)} / ${format.size(total)} GB`);
    }
    const [first] = meters.gpus;
    if (toggles.gpu && first !== undefined) {
        parts.push(`GPU ${percent(first.busiest_pct)}`);
    }
    return parts.join(" · ");
}
/**
 * @param {UiState} state
 * @returns {Reservations}
 */
function reservationsOf(state) {
    const { meters } = state;
    const any = meters.cpu.reserved_cores > 0 ||
        meters.ram.reserved_mb > 0 ||
        meters.gpus.some((gpu) => gpu.reserved_pct > 0 || gpu.reserved_vram_mb > 0);
    return {
        benchmark: state.active.some((job) => job.lease.kind === "benchmark_exclusive"),
        none: any ? "Not reserved" : "No reservations",
    };
}
/**
 * @param {number} reserved
 * @returns {number | null}
 */
function tick(reserved) {
    return reserved > 0 ? reserved : null;
}
/**
 * @param {number} part
 * @param {number} whole
 * @returns {number}
 */
function fraction(part, whole) {
    return whole > 0 ? clamp(part / whole) : 0;
}
/**
 * @param {number} value
 * @returns {number}
 */
function clamp(value) {
    return Math.min(Math.max(value, 0), 1);
}
/**
 * @param {number} pct
 * @returns {string}
 */
function percent(pct) {
    return `${format.whole(pct)}%`;
}
/**
 * @param {number[]} values
 * @returns {number}
 */
function sum(values) {
    return values.reduce((total, value) => total + value, 0);
}
