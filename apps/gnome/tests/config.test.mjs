import assert from "node:assert/strict";
import { test } from "node:test";

import { dataPaths } from "../allot@mehran.tech/lib/config.js";

test("data paths sit under ~/.allot", () => {
  assert.deepEqual(dataPaths("/home/user/"), {
    root: "/home/user/.allot",
    config: "/home/user/.allot/config.toml",
    logs: "/home/user/.allot/logs",
    settings: "/home/user/.allot/tray.json",
  });
});
