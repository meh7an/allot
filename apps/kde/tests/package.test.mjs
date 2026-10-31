import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";

import { INDICATOR_ICONS, iconFile } from "../../gnome/allot@mehran.tech/lib/indicator-status.js";
import { FakeTimers, fixtureText, settle } from "../../gnome/tests/support.mjs";
import { CODE_DIR, emit } from "../tools/emit.mjs";
import { PACKAGE_DIR, catAnswer, fakeRequests } from "./support.mjs";

/** @param {URL} dir @param {string} [prefix] @returns {string[]} */
function filesUnder(dir, prefix = "") {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory()
      ? filesUnder(new URL(`${entry.name}/`, dir), `${prefix}${entry.name}/`)
      : [`${prefix}${entry.name}`],
  );
}

test("the generated code is current: run pnpm build after changing its sources", () => {
  const expected = emit();

  assert.deepEqual(filesUnder(CODE_DIR).sort(), [...expected.keys()].sort());
  for (const [path, contents] of expected) {
    assert.equal(readFileSync(new URL(path, CODE_DIR), "utf8"), contents, path);
  }
});

test("metadata names the plugin id and the Plasma 6.0 API", () => {
  const metadata = JSON.parse(readFileSync(new URL("metadata.json", PACKAGE_DIR), "utf8"));

  assert.equal(metadata.KPlugin.Id, "tech.mehran.allot");
  assert.equal(metadata.KPackageStructure, "Plasma/Applet");
  assert.equal(metadata["X-Plasma-API-Minimum-Version"], "6.0");
});

test("every panel state and the header mark has a recolourable icon", () => {
  const icons = new URL("contents/icons/", PACKAGE_DIR);
  const files = [...INDICATOR_ICONS.map(iconFile), "allot-mark-symbolic.svg"];

  assert.deepEqual(readdirSync(icons).sort(), [...files].sort());
  for (const file of files) {
    const svg = readFileSync(new URL(file, icons), "utf8");
    assert.match(svg, /<style id="current-color-scheme"/, file);
    assert.doesNotMatch(svg, /#2e3436|#4e9a06|#f57900|#cc0000/, `${file} keeps a GNOME colour`);
  }
});

test("the generated widget module runs a poll", async () => {
  const { AllotWidget } = await import(new URL("widget.mjs", CODE_DIR).href);
  const timers = new FakeTimers();
  const body = fixtureText("ui-state-idle.json");
  /** @type {string[]} */
  const summaries = [];
  /** @type {any} */
  let widget = null;
  widget = new AllotWidget({
    homeUrl: "file:///home/u",
    connectSource: (/** @type {string} */ command) =>
      queueMicrotask(() =>
        widget.deliver(command, catAnswer(command, { "/home/u/.allot/token": "t" })),
      ),
    requests: fakeRequests(() => ({ status: 200, body })),
    timers,
    onChange: () => summaries.push(widget.model.snapshot.header.summary),
    log: () => {},
  });

  widget.start();
  await settle();
  widget.stop();

  assert.deepEqual(summaries, ["Idle"]);
});
