// Generated from apps/kde/src/rows.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/**
 * @import {
 *   JobGroup,
 *   JobRow,
 *   PanelSnapshot,
 *   RecentLine,
 * } from "./shared/snapshot.js"
 */
var __classPrivateFieldSet = (this && this.__classPrivateFieldSet) || function (receiver, state, value, kind, f) {
    if (kind === "m") throw new TypeError("Private method is not writable");
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
    return (kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value)), value;
};
var __classPrivateFieldGet = (this && this.__classPrivateFieldGet) || function (receiver, state, kind, f) {
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
    return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
};
var _RowKeeper_instances, _RowKeeper_model, _RowKeeper_rows, _RowKeeper_merged, _RowKeeper_place, _RowKeeper_indexOf;
/**
 * One row of the popup's list. The key stays the same while the row is shown, so its view keeps
 * its focus and animations between ticks.
 *
 * @typedef {(
 *   | { key: string, kind: "heading", group: JobGroup }
 *   | { key: string, kind: "job", job: JobRow, icon: string | null }
 *   | { key: string, kind: "idle", text: string }
 *   | { key: string, kind: "recent", lines: RecentLine[] }
 *   | { key: string, kind: "nothing", text: string }
 * )} Row
 */
/**
 * The part of QML's ListModel the keeper uses.
 *
 * @typedef {object} RowModel
 * @property {number} count
 * @property {(index: number) => { key: string, leaving: boolean }} get
 * @property {(index: number, row: object) => void} insert
 * @property {(from: number, to: number, count: number) => void} move
 * @property {(index: number, count: number) => void} remove
 * @property {(index: number, role: string, value: boolean) => void} setProperty
 */
/**
 * The popup's rows for a snapshot, top to bottom. The first-run card comes with M5.6.
 *
 * @param {PanelSnapshot} snapshot
 * @param {(exe: string | null) => string | null} appIcon
 * @param {{ idle: string, nothing: string }} text The idle line's text and the empty line's.
 * @returns {Row[]}
 */
export function listRows(snapshot, appIcon, text) {
    /** @type {Row[]} */
    const rows = [];
    for (const group of snapshot.groups) {
        rows.push({ key: `group:${group.kind}`, kind: "heading", group });
        for (const job of group.jobs) {
            const icon = job.card.isQueued ? null : appIcon(job.card.exePath);
            rows.push({ key: `job:${job.id}`, kind: "job", job, icon });
        }
    }
    if (snapshot.idle !== null) {
        rows.push({ key: "idle", kind: "idle", text: text.idle });
    }
    if (snapshot.recent.length > 0) {
        rows.push({ key: "recent", kind: "recent", lines: snapshot.recent });
    }
    else if (snapshot.showNothing) {
        rows.push({ key: "nothing", kind: "nothing", text: text.nothing });
    }
    return rows;
}
/**
 * Keeps a ListModel of `{ key, kind, leaving, entering }` in step with the rows, so that a view is
 * reused for as long as its key is shown. A row whose key is gone stays where it was with
 * `leaving` set until its view has collapsed and calls `gone`, so the rows around it do not jump.
 */
export class RowKeeper {
    /** @param {RowModel} model */
    constructor(model) {
        _RowKeeper_instances.add(this);
        _RowKeeper_model.set(this, void 0);
        /** @type {Map<string, Row>} */
        _RowKeeper_rows.set(this, new Map());
        __classPrivateFieldSet(this, _RowKeeper_model, model, "f");
    }
    /**
     * @param {Row[]} rows
     * @param {boolean} animate Rows enter and leave with their animation; off when the popup opens.
     * @param {(data: Record<string, Row>) => void} publish Hands every shown row's data, by key, to
     *   the views. Called before the model changes, because a new view reads its row at once.
     */
    show(rows, animate, publish) {
        const order = __classPrivateFieldGet(this, _RowKeeper_instances, "m", _RowKeeper_merged).call(this, rows, animate);
        /** @type {Map<string, Row>} */
        const shown = new Map();
        /** @type {Record<string, Row>} */
        const data = {};
        for (const row of rows) {
            __classPrivateFieldGet(this, _RowKeeper_rows, "f").set(row.key, row);
        }
        for (const key of order) {
            const row = /** @type {Row} */ (__classPrivateFieldGet(this, _RowKeeper_rows, "f").get(key));
            shown.set(key, row);
            data[key] = row;
        }
        __classPrivateFieldSet(this, _RowKeeper_rows, shown, "f");
        publish(data);
        const wanted = new Set(rows.map((row) => row.key));
        order.forEach((key, index) => {
            __classPrivateFieldGet(this, _RowKeeper_instances, "m", _RowKeeper_place).call(this, key, index, wanted.has(key), animate);
        });
        if (__classPrivateFieldGet(this, _RowKeeper_model, "f").count > order.length) {
            __classPrivateFieldGet(this, _RowKeeper_model, "f").remove(order.length, __classPrivateFieldGet(this, _RowKeeper_model, "f").count - order.length);
        }
    }
    /**
     * A leaving row's view has collapsed.
     *
     * @param {string} key
     */
    gone(key) {
        const index = __classPrivateFieldGet(this, _RowKeeper_instances, "m", _RowKeeper_indexOf).call(this, key);
        if (index >= 0 && __classPrivateFieldGet(this, _RowKeeper_model, "f").get(index).leaving) {
            __classPrivateFieldGet(this, _RowKeeper_model, "f").remove(index, 1);
            __classPrivateFieldGet(this, _RowKeeper_rows, "f").delete(key);
        }
    }
}
_RowKeeper_model = new WeakMap(), _RowKeeper_rows = new WeakMap(), _RowKeeper_instances = new WeakSet(), _RowKeeper_merged = function _RowKeeper_merged(rows, animate) {
    const order = rows.map((row) => row.key);
    if (!animate) {
        return order;
    }
    /** @type {string | null} */
    let previous = null;
    for (let index = 0; index < __classPrivateFieldGet(this, _RowKeeper_model, "f").count; index += 1) {
        const { key } = __classPrivateFieldGet(this, _RowKeeper_model, "f").get(index);
        if (!order.includes(key)) {
            order.splice(previous === null ? 0 : order.indexOf(previous) + 1, 0, key);
        }
        previous = key;
    }
    return order;
}, _RowKeeper_place = function _RowKeeper_place(key, index, wanted, animate) {
    const found = __classPrivateFieldGet(this, _RowKeeper_instances, "m", _RowKeeper_indexOf).call(this, key);
    if (found < 0) {
        const { kind } = /** @type {Row} */ (__classPrivateFieldGet(this, _RowKeeper_rows, "f").get(key));
        __classPrivateFieldGet(this, _RowKeeper_model, "f").insert(index, { key, kind, leaving: false, entering: animate });
        return;
    }
    if (found !== index) {
        __classPrivateFieldGet(this, _RowKeeper_model, "f").move(found, index, 1);
    }
    if (__classPrivateFieldGet(this, _RowKeeper_model, "f").get(index).leaving === wanted) {
        __classPrivateFieldGet(this, _RowKeeper_model, "f").setProperty(index, "leaving", !wanted);
    }
}, _RowKeeper_indexOf = function _RowKeeper_indexOf(key) {
    for (let index = 0; index < __classPrivateFieldGet(this, _RowKeeper_model, "f").count; index += 1) {
        if (__classPrivateFieldGet(this, _RowKeeper_model, "f").get(index).key === key) {
            return index;
        }
    }
    return -1;
};
