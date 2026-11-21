// The value-to-text cases every port runs: contract/text-fixtures/live-text.json. Memory and
// percentages round half away from zero, so 256 MB reads 0.3 GB and 12.5 % reads 13.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { estimatesText, fitsCaption, headroomText } from "../allot@mehran.tech/lib/live-text.js";
import { localeNumbers } from "../allot@mehran.tech/lib/settings-text.js";
import { REPO_DIR } from "./support.mjs";

const cases = JSON.parse(
  readFileSync(new URL("contract/text-fixtures/live-text.json", REPO_DIR), "utf8"),
);
const number = localeNumbers(cases.locale);

test("numbers read as the fixture says", () => {
  for (const entry of cases.numbers) {
    assert.equal(number(entry.value), entry.text);
  }
});

test("headroom reads memory in gigabytes as the fixture says", () => {
  for (const entry of cases.headroom) {
    assert.equal(headroomText(entry.kept, number), entry.text);
  }
});

test("the learned-estimate sentence reserves cores as the fixture says", () => {
  for (const entry of cases.estimates) {
    const values = { lower: true, minRuns: 3, marginPct: entry.margin_pct };
    assert.equal(estimatesText(values, number), entry.text);
  }
});

test("the What fits caption reads the busy share as the fixture says", () => {
  for (const entry of cases.fits_caption) {
    assert.equal(fitsCaption(entry.cpu_busy_pct, number), entry.text);
  }
});
