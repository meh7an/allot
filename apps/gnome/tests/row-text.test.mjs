import assert from "node:assert/strict";
import { test } from "node:test";

import {
  formatRowNumber,
  markupEscaped,
  rowDescription,
} from "../allot@mehran.tech/lib/row-text.js";
import { localeNumbers } from "../allot@mehran.tech/lib/settings-text.js";

test("markup escaping covers the five characters both toolkits read", () => {
  assert.equal(
    markupEscaped(`a < b & "c" > 'd'`),
    "a &lt; b &amp; &quot;c&quot; &gt; &#39;d&#39;",
  );
  assert.equal(markupEscaped("&lt;"), "&amp;lt;");
  assert.equal(markupEscaped("plain"), "plain");
});

test("a description reads the help, the key, the notes, then the error", () => {
  assert.equal(rowDescription("Help.", "a.b"), "Help. Key: a.b.");
  assert.equal(rowDescription("Help.", null), "Help.");
  assert.equal(rowDescription("Help.", ""), "Help.");
  assert.equal(
    rowDescription("Help.", "a.b", ["Note one.", null, "", "Note two."], "Too big."),
    "Help. Key: a.b. Note one. Note two. Error: Too big.",
  );
  assert.equal(rowDescription("Help.", null, [], ""), "Help.");
});

/** @param {number} value @param {number} decimals */
const fixed = (value, decimals) => `${value.toFixed(decimals)}|${decimals}`;

test("a row number rounds to two decimals and shows only the decimals it needs", () => {
  assert.equal(formatRowNumber(3, fixed), "3|0");
  assert.equal(formatRowNumber(3.999, fixed), "4|0");
  assert.equal(formatRowNumber(2.5, fixed), "2.5|1");
  assert.equal(formatRowNumber(0.125, fixed), "0.13|2");
  assert.equal(formatRowNumber(12000, fixed), "12000|0");
});

test("GNOME's locale numbers go through the same helper, grouped", () => {
  const number = localeNumbers("en-US");

  assert.equal(number(12000), "12,000");
  assert.equal(number(2.5), "2.5");
  assert.equal(number(0.125), "0.13");
  assert.equal(number(3.999), "4");
});
