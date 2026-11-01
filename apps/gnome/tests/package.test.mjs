import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { EXTENSION_DIR, REPO_DIR } from "./support.mjs";

test("the contract module is an unchanged copy of the generated one", () => {
  const generated = readFileSync(new URL("contract/typescript/contract.g.mjs", REPO_DIR));
  const copy = readFileSync(new URL("contract.g.mjs", EXTENSION_DIR));

  assert.ok(
    copy.equals(generated),
    "copy contract/typescript/contract.g.mjs to apps/gnome/allot@mehran.tech/",
  );
});

test("metadata names the folder's UUID and GNOME Shell 48 to 51", () => {
  const metadata = JSON.parse(readFileSync(new URL("metadata.json", EXTENSION_DIR), "utf8"));

  assert.equal(metadata.uuid, "allot@mehran.tech");
  assert.deepEqual(metadata["shell-version"], ["48", "49", "50", "51"]);
});

test("the high-contrast sheet styles only classes the popup has", () => {
  const css = (name) => readFileSync(new URL(name, EXTENSION_DIR), "utf8");
  const known = css("stylesheet.css") + css("stylesheet-dark.css");
  const sources = ["cards.js", "popup.js", "resources-card.js"].map(css).join("");
  const classes = new Set(css("high-contrast.css").match(/\.allot-[a-z-]+/g));

  assert.ok(classes.size > 0);
  for (const name of classes) {
    const bare = name.slice(1);
    assert.ok(known.includes(name) || sources.includes(bare), `nothing has ${name}`);
  }
});
