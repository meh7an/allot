import assert from "node:assert/strict";
import { test } from "node:test";

import { panelSnapshot } from "../../gnome/allot@mehran.tech/lib/snapshot.js";
import { scenario } from "../../gnome/tests/handoff.mjs";
import { RowKeeper, listRows } from "../src/rows.js";
import { FakeRowModel } from "./support.mjs";

/** @import { Row } from "../src/rows.js" */

const TEXT = { idle: "Idle · 1", nothing: "Nothing" };

/**
 * @param {string} name A handoff scenario.
 * @param {(exe: string | null) => string | null} [appIcon]
 * @returns {Row[]}
 */
function rowsOf(name, appIcon = () => null) {
  const { status, state } = scenario(name);
  return listRows(panelSnapshot(status, state), appIcon, TEXT);
}

/**
 * @param {string[]} keys
 * @returns {Row[]}
 */
function lines(...keys) {
  return keys.map((key) => ({ key, kind: "idle", text: key }));
}

function keeper() {
  const model = new FakeRowModel();
  /** @type {Record<string, Row>[]} */
  const published = [];
  const subject = new RowKeeper(model);
  /**
   * @param {Row[]} rows
   * @param {boolean} [animate]
   */
  const show = (rows, animate = true) => {
    subject.show(rows, animate, (data) => published.push(data));
  };
  return { subject, model, published, show };
}

test("the rows follow the snapshot: heading, then its jobs, then the idle line", () => {
  const keys = rowsOf("several").map((row) => row.key);

  assert.deepEqual(keys.slice(0, 1), ["group:running"]);
  assert.ok(keys.indexOf("group:waiting") > keys.indexOf("group:running"));
  assert.equal(keys[keys.length - 1], "idle");
  assert.equal(keys.filter((key) => key.startsWith("job:")).length, 3);
});

test("Recently finished replaces the empty line", () => {
  assert.deepEqual(
    rowsOf("empty").map((row) => row.kind),
    ["recent"],
  );
  const { status, state } = scenario("first-run");

  assert.deepEqual(
    listRows(panelSnapshot(status, { ...state, recent: [] }), () => null, TEXT).map(
      (row) => row.kind,
    ),
    ["nothing"],
  );
});

test("the first-run card follows the idle line and replaces the empty line", () => {
  const { status, state } = scenario("first-run");
  const options = { firstRun: { isDone: false, doNotDisturb: true } };

  const rows = listRows(panelSnapshot(status, state, options), () => null, TEXT, true);

  assert.deepEqual(rows.at(-1), {
    key: "welcome",
    kind: "welcome",
    doNotDisturb: true,
    notificationsOff: true,
  });
  assert.ok(!rows.some((row) => row.kind === "nothing"));
});

test("a queued card asks for no app icon, a running one for its executable's", () => {
  /** @type {(string | null)[]} */
  const asked = [];
  const rows = rowsOf("icons", (exe) => {
    asked.push(exe);
    return exe === null ? null : "app";
  });
  const icons = rows.flatMap((row) => (row.kind === "job" ? [row.icon] : []));

  assert.equal(asked.length, 4, "four running, two queued");
  assert.deepEqual(icons.slice(0, 4), ["app", "app", "app", "app"]);
  assert.deepEqual(icons.slice(4), [null, null]);
});

test("showing the same keys again keeps every view", () => {
  const { model, show } = keeper();

  show(lines("a", "b"), false);
  show(lines("a", "b"));
  show(lines("a", "b"));

  assert.deepEqual(model.keys, ["a", "b"]);
  assert.equal(model.inserted, 2);
});

test("the views can read a new row's data before it is inserted", () => {
  const model = new FakeRowModel();
  let countWhenPublished = -1;

  new RowKeeper(model).show(lines("a"), true, (data) => {
    countWhenPublished = model.count;
    assert.equal(data.a?.key, "a");
  });

  assert.equal(countWhenPublished, 0);
  assert.equal(model.count, 1);
});

test("a row enters animated only when the list animates", () => {
  const { model, show } = keeper();

  show(lines("a"), false);
  show(lines("a", "b"));

  assert.equal(model.get(0).entering, false);
  assert.equal(model.get(1).entering, true);
});

test("a row that went stays where it was, with its data, until its view has collapsed", () => {
  const { subject, model, published, show } = keeper();
  show(lines("a", "b", "c"));

  show(lines("a", "c"));

  assert.deepEqual(model.keys, ["a", "b", "c"]);
  assert.equal(model.isLeaving("b"), true);
  assert.equal(published[published.length - 1]?.b?.key, "b");

  subject.gone("b");
  show(lines("a", "c"));

  assert.deepEqual(model.keys, ["a", "c"]);
  assert.equal(published[published.length - 1]?.b, undefined);
});

test("a first row that went stays first", () => {
  const { model, show } = keeper();
  show(lines("a", "b"));

  show(lines("b"));

  assert.deepEqual(model.keys, ["a", "b"]);
});

test("a row that comes back while leaving keeps its view", () => {
  const { model, show } = keeper();
  show(lines("a", "b"));
  show(lines("a"));

  show(lines("a", "b"));

  assert.equal(model.isLeaving("b"), false);
  assert.equal(model.inserted, 2);
});

test("only a leaving row can go", () => {
  const { subject, model, show } = keeper();
  show(lines("a"));

  subject.gone("a");
  subject.gone("missing");

  assert.deepEqual(model.keys, ["a"]);
});

test("rows follow a new order without new views", () => {
  const { model, show } = keeper();
  show(lines("a", "b", "c"));

  show(lines("c", "a", "b"));

  assert.deepEqual(model.keys, ["c", "a", "b"]);
  assert.equal(model.inserted, 3);
});

test("showing without animation drops what went, including rows still leaving", () => {
  const { model, show } = keeper();
  show(lines("a", "b", "c"));
  show(lines("a", "c"));

  show(lines("c"), false);

  assert.deepEqual(model.keys, ["c"]);
});
