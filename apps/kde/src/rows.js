/**
 * @import {
 *   JobGroup,
 *   JobRow,
 *   PanelSnapshot,
 *   RecentLine,
 * } from "../../gnome/allot@mehran.tech/lib/snapshot.js"
 */

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
  } else if (snapshot.showNothing) {
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
  #model;
  /** @type {Map<string, Row>} */
  #rows = new Map();

  /** @param {RowModel} model */
  constructor(model) {
    this.#model = model;
  }

  /**
   * @param {Row[]} rows
   * @param {boolean} animate Rows enter and leave with their animation; off when the popup opens.
   * @param {(data: Record<string, Row>) => void} publish Hands every shown row's data, by key, to
   *   the views. Called before the model changes, because a new view reads its row at once.
   */
  show(rows, animate, publish) {
    const order = this.#merged(rows, animate);
    /** @type {Map<string, Row>} */
    const shown = new Map();
    /** @type {Record<string, Row>} */
    const data = {};
    for (const row of rows) {
      this.#rows.set(row.key, row);
    }
    for (const key of order) {
      const row = /** @type {Row} */ (this.#rows.get(key));
      shown.set(key, row);
      data[key] = row;
    }
    this.#rows = shown;
    publish(data);

    const wanted = new Set(rows.map((row) => row.key));
    order.forEach((key, index) => {
      this.#place(key, index, wanted.has(key), animate);
    });
    if (this.#model.count > order.length) {
      this.#model.remove(order.length, this.#model.count - order.length);
    }
  }

  /**
   * A leaving row's view has collapsed.
   *
   * @param {string} key
   */
  gone(key) {
    const index = this.#indexOf(key);
    if (index >= 0 && this.#model.get(index).leaving) {
      this.#model.remove(index, 1);
      this.#rows.delete(key);
    }
  }

  /**
   * The keys to show: the rows', with each leaving row after the row it followed.
   *
   * @param {Row[]} rows
   * @param {boolean} animate
   * @returns {string[]}
   */
  #merged(rows, animate) {
    const order = rows.map((row) => row.key);
    if (!animate) {
      return order;
    }
    /** @type {string | null} */
    let previous = null;
    for (let index = 0; index < this.#model.count; index += 1) {
      const { key } = this.#model.get(index);
      if (!order.includes(key)) {
        order.splice(previous === null ? 0 : order.indexOf(previous) + 1, 0, key);
      }
      previous = key;
    }
    return order;
  }

  /**
   * @param {string} key
   * @param {number} index
   * @param {boolean} wanted
   * @param {boolean} animate
   */
  #place(key, index, wanted, animate) {
    const found = this.#indexOf(key);
    if (found < 0) {
      const { kind } = /** @type {Row} */ (this.#rows.get(key));
      this.#model.insert(index, { key, kind, leaving: false, entering: animate });
      return;
    }
    if (found !== index) {
      this.#model.move(found, index, 1);
    }
    if (this.#model.get(index).leaving === wanted) {
      this.#model.setProperty(index, "leaving", !wanted);
    }
  }

  /**
   * @param {string} key
   * @returns {number}
   */
  #indexOf(key) {
    for (let index = 0; index < this.#model.count; index += 1) {
      if (this.#model.get(index).key === key) {
        return index;
      }
    }
    return -1;
  }
}
