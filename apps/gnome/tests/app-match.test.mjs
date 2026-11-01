import assert from "node:assert/strict";
import { test } from "node:test";

import { execProgram, matchingApp, sandboxedAppId } from "../allot@mehran.tech/lib/app-match.js";

const APPS = [
  { id: "code.desktop", exec: "/usr/share/code/code %F", tryExec: "" },
  { id: "org.gnome.Nautilus.desktop", exec: "nautilus --new-window %U", tryExec: "nautilus" },
  { id: "idle.desktop", exec: "python3 /usr/share/idle/idle.py", tryExec: "" },
  { id: "launcher.desktop", exec: "env GDK_BACKEND=x11 LANG=C blender %f", tryExec: "" },
];

test("sandboxed apps are named by their install path", () => {
  assert.equal(
    sandboxedAppId("/var/lib/flatpak/app/org.gimp.GIMP/x86_64/stable/active/files/bin/gimp"),
    "org.gimp.GIMP.desktop",
  );
  assert.equal(
    sandboxedAppId("/home/user/.local/share/flatpak/app/com.slack.Slack/current/files/slack"),
    "com.slack.Slack.desktop",
  );
  assert.equal(
    sandboxedAppId("/snap/firefox/4793/usr/lib/firefox/firefox"),
    "firefox_firefox.desktop",
  );
  assert.equal(sandboxedAppId("/usr/bin/cargo"), null);
});

test("the Exec program skips env and its variables", () => {
  assert.equal(execProgram("env GDK_BACKEND=x11 LANG=C blender %f"), "blender");
  assert.equal(execProgram("/usr/bin/env FOO=1 code"), "code");
  assert.equal(execProgram("  nautilus --new-window %U"), "nautilus");
  assert.equal(execProgram(""), "");
});

test("an executable matches an app by full path or, on PATH, by name", () => {
  assert.equal(matchingApp("/usr/share/code/code", APPS), "code.desktop");
  assert.equal(matchingApp("/usr/local/share/code/code", APPS), null);
  assert.equal(matchingApp("/usr/bin/nautilus", APPS), "org.gnome.Nautilus.desktop");
  assert.equal(matchingApp("/usr/bin/blender", APPS), "launcher.desktop");
});

test("command-line tools and interpreters keep their initials", () => {
  assert.equal(matchingApp("/home/user/.cargo/bin/cargo", APPS), null);
  assert.equal(matchingApp("/usr/bin/python3", APPS), null);
  const versioned = [{ id: "python3.14.desktop", exec: "/usr/bin/python3.14", tryExec: "" }];
  assert.equal(matchingApp("/usr/bin/python3.14", versioned), null);
  assert.equal(matchingApp("", APPS), null);
});
