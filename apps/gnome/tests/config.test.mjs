import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_PORT, dataPaths, portFromToml } from "../allot@mehran.tech/lib/config.js";

test("port comes from [daemon] or a dotted key, else the default", () => {
  const cases = [
    ["[daemon]\nport = 50000\n", 50000],
    ["[daemon]\nport = 47_900 # moved\n", 47900],
    ["daemon.port = 51000\n", 51000],
    ["[daemon]\r\nport = 52000\r\n", 52000],
    ["[ daemon ]\nport=53000", 53000],
    ["[admission]\nport = 50000\n", DEFAULT_PORT],
    ["[daemon]\nport = 0\n", DEFAULT_PORT],
    ["[daemon]\nport = 70000\n", DEFAULT_PORT],
    ['[daemon]\nport = "50000"\n', DEFAULT_PORT],
    ["[daemon]\nport = -1\n", DEFAULT_PORT],
    ["", DEFAULT_PORT],
  ];
  for (const [toml, expected] of cases) {
    assert.equal(portFromToml(String(toml)), expected, JSON.stringify(toml));
  }
});

test("data paths sit under ~/.allot", () => {
  assert.deepEqual(dataPaths("/home/user/"), {
    root: "/home/user/.allot",
    token: "/home/user/.allot/token",
    config: "/home/user/.allot/config.toml",
    logs: "/home/user/.allot/logs",
  });
});
