import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { settle } from "../../gnome/tests/support.mjs";
import {
  AppIcons,
  DESKTOP_ENTRIES_COMMAND,
  iconSource,
  parseDesktopEntries,
} from "../src/app-icons.js";
import { CommandRunner } from "../src/io.js";

const OUTPUT = [
  "--",
  "./org.kde.dolphin.desktop:Type=Application",
  "./org.kde.dolphin.desktop:Exec=dolphin %u",
  "./org.kde.dolphin.desktop:Icon=system-file-manager",
  "./org.kde.dolphin.desktop:Exec=dolphin --new-window",
  "./kde/konsole.desktop:Type=Application",
  "./kde/konsole.desktop:Exec=konsole",
  "./kde/konsole.desktop:TryExec=/usr/bin/konsole",
  "./kde/konsole.desktop:Icon=/opt/konsole.png",
  "./helper.desktop:Type=Application",
  "./helper.desktop:Exec=helper",
  "./helper.desktop:NoDisplay=true",
  "./masked.desktop:Type=Application",
  "./masked.desktop:Exec=masked",
  "./masked.desktop:Hidden=true",
  "./link.desktop:Type=Link",
  "--",
  "./masked.desktop:Type=Application",
  "./masked.desktop:Exec=masked",
  "./org.kde.dolphin.desktop:Icon=other",
  "./org.gimp.GIMP.desktop:Type=Application",
  "./org.gimp.GIMP.desktop:Exec=gimp-3.0 %U",
  "./org.gimp.GIMP.desktop:Icon=gimp",
  "./gimp_gimp.desktop:Type=Application",
  "./gimp_gimp.desktop:Exec=/snap/bin/gimp",
  "./gimp_gimp.desktop:Icon=snap-gimp",
  "",
].join("\n");

/** @param {string} [output] */
function subject(output = OUTPUT) {
  let loaded = 0;
  /** @type {string[]} */
  const commands = [];
  const runner = new CommandRunner((command) => {
    commands.push(command);
    queueMicrotask(() => runner.deliver(command, { "exit code": 0, stdout: output }));
  });
  const icons = new AppIcons(runner, () => {
    loaded += 1;
  });
  return {
    icons,
    commands,
    get loaded() {
      return loaded;
    },
  };
}

test("only an app a user can launch is listed", () => {
  const ids = parseDesktopEntries(OUTPUT).map((app) => app.id);

  assert.deepEqual(ids, [
    "org.kde.dolphin.desktop",
    "kde-konsole.desktop",
    "org.gimp.GIMP.desktop",
    "gimp_gimp.desktop",
  ]);
});

test("an entry's own Exec counts, not an action's", () => {
  const [dolphin] = parseDesktopEntries(OUTPUT);

  assert.equal(dolphin?.exec, "dolphin %u");
  assert.equal(dolphin?.icon, "system-file-manager", "an earlier directory wins");
});

test("an icon is a theme name or a file URL", () => {
  assert.equal(iconSource("gimp"), "gimp");
  assert.equal(iconSource("/opt/konsole.png"), "file:///opt/konsole.png");
  assert.equal(iconSource(""), null);
});

test("nothing is read until a card has an executable", async () => {
  const context = subject();

  assert.equal(context.icons.icon(null), null);
  assert.equal(context.icons.icon("cargo"), null);
  await settle();

  assert.deepEqual(context.commands, []);
});

test("the first card starts one read, and every card shows once it is done", async () => {
  const context = subject();

  assert.equal(context.icons.icon("/usr/bin/dolphin"), null);
  assert.equal(context.icons.icon("/usr/bin/konsole"), null);
  await settle();

  assert.deepEqual(context.commands, [DESKTOP_ENTRIES_COMMAND]);
  assert.equal(context.loaded, 1);
  assert.equal(context.icons.icon("/usr/bin/dolphin"), "system-file-manager");
  assert.equal(context.icons.icon("/usr/bin/konsole"), "file:///opt/konsole.png");
  assert.equal(context.icons.icon("/usr/bin/cargo"), null);
});

test("a sandboxed app is found by its ID, and a launcher never matches", async () => {
  const context = subject();
  context.icons.icon("/usr/bin/dolphin");
  await settle();

  assert.equal(
    context.icons.icon("/var/lib/flatpak/app/org.gimp.GIMP/x86_64/stable/files/bin/gimp-3.0"),
    "gimp",
  );
  assert.equal(context.icons.icon("/snap/gimp/123/usr/bin/gimp"), "snap-gimp");
  assert.equal(context.icons.icon("/usr/bin/python3.14"), null);
});

test("closing the popup forgets the entries, so the next opening reads them again", async () => {
  const context = subject();
  context.icons.icon("/usr/bin/dolphin");
  await settle();

  context.icons.forget();
  assert.equal(context.icons.icon("/usr/bin/dolphin"), null);
  await settle();

  assert.equal(context.commands.length, 2);
  assert.equal(context.icons.icon("/usr/bin/dolphin"), "system-file-manager");
});

test(
  "the command prints each data directory's entries with their relative paths",
  { skip: process.platform === "win32" && "needs a POSIX shell" },
  () => {
    const root = mkdtempSync(join(tmpdir(), "allot-entries-"));
    try {
      const home = join(root, "home data");
      mkdirSync(join(home, "applications", "kde"), { recursive: true });
      mkdirSync(join(root, "system", "applications"), { recursive: true });
      const body = "[Desktop Entry]\nType=Application\nName=Ignored\nExec=one\nIcon=first\n";
      writeFileSync(join(home, "applications", "kde", "one.desktop"), body);
      writeFileSync(
        join(root, "system", "applications", "two.desktop"),
        "[Desktop Entry]\nType=Application\nExec=two\n[Desktop Action x]\nExec=two --x\n",
      );
      writeFileSync(join(root, "system", "applications", "notes.txt"), "Exec=no\n");

      const result = spawnSync("sh", ["-c", DESKTOP_ENTRIES_COMMAND], {
        encoding: "utf8",
        env: {
          ...process.env,
          XDG_DATA_HOME: home,
          XDG_DATA_DIRS: `${join(root, "missing")}:${join(root, "system")}`,
        },
      });

      assert.deepEqual(parseDesktopEntries(result.stdout), [
        { id: "kde-one.desktop", exec: "one", tryExec: "", icon: "first" },
        { id: "two.desktop", exec: "two", tryExec: "", icon: "" },
      ]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);
