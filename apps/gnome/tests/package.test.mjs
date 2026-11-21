import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
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

test("the extension reaches allotd over D-Bus only: no libsoup anywhere in it", () => {
  const files = readdirSync(EXTENSION_DIR, { recursive: true, encoding: "utf8" });
  const scripts = files.filter((name) => name.endsWith(".js") || name.endsWith(".mjs"));

  assert.ok(scripts.length > 0);
  for (const name of scripts) {
    const text = readFileSync(new URL(name.replaceAll("\\", "/"), EXTENSION_DIR), "utf8");
    assert.doesNotMatch(text, /soup/i, `${name} mentions libsoup`);
  }
});

// The zip is `git archive` of the extension folder, so every file in it ships. allotd is reached
// over D-Bus only: no loopback address, no plain-HTTP URL and no bearer key may be left behind.
const FORBIDDEN = [
  [/127\.0\.0\.1/i, "a loopback address"],
  [/http:\/\//i, "a plain-HTTP URL"],
  [/bearer/i, "a bearer"],
  [/token/i, "a token"],
];

/** The SVG namespace is an identifier, not a URL anything fetches. */
const SVG_NAMESPACE = "http://www.w3.org/2000/svg";

test("no shipped file talks HTTP or handles a token", () => {
  const files = readdirSync(EXTENSION_DIR, { recursive: true, encoding: "utf8" }).filter(
    (name) => statSync(new URL(name.replaceAll("\\", "/"), EXTENSION_DIR)).isFile(),
  );

  assert.ok(files.length > 30);
  for (const name of files) {
    const text = readFileSync(new URL(name.replaceAll("\\", "/"), EXTENSION_DIR), "utf8");
    const checked = text.replaceAll(SVG_NAMESPACE, "");
    for (const [pattern, what] of FORBIDDEN) {
      assert.doesNotMatch(checked, pattern, `${name} mentions ${what}`);
    }
  }
});

test("the folder carries the license the package is published under", () => {
  const license = readFileSync(new URL("LICENSE", EXTENSION_DIR), "utf8");
  const root = readFileSync(new URL("LICENSE-MIT", REPO_DIR), "utf8");

  assert.equal(license, root);
});

test("a shipped script names no design document, milestone or person", () => {
  const files = readdirSync(EXTENSION_DIR, { recursive: true, encoding: "utf8" }).filter(
    (name) => name.endsWith(".js") && !name.endsWith("contract.g.mjs"),
  );

  for (const name of files) {
    const text = readFileSync(new URL(name.replaceAll("\\", "/"), EXTENSION_DIR), "utf8");
    assert.doesNotMatch(text, /DESIGN\.md|handoff section|\bM\d\.\d\b|\bD\d{1,2}\b|Mehran/, name);
  }
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
