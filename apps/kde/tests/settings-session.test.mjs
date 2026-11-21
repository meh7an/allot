// The settings session as the widget runs it: the lowered shared modules over XMLHttpRequest.

import assert from "node:assert/strict";
import { test } from "node:test";

import { FakeTimers, fixtureText, settle } from "../../gnome/tests/support.mjs";
import { settingsBanners } from "../tech.mehran.allot/contents/code/shared/settings-banners.js";
import { SettingsSession } from "../tech.mehran.allot/contents/code/shared/settings-session.js";
import { DaemonClient } from "../tech.mehran.allot/contents/code/http-client.js";
import { XhrTransport } from "../tech.mehran.allot/contents/code/io.js";
import { fakeRequests } from "./support.mjs";

/** @import { FakeXhr } from "./support.mjs" */

const STATE = fixtureText("settings-state.json");

/**
 * A session on the widget's transport, with allotd answering the settings-state fixture to
 * every read and write, or not answering at all while `up` is false. A `status` other than 200
 * answers every request with it and an empty body.
 */
async function widgetSession(up = true, status = 200) {
  const timers = new FakeTimers();
  /** @type {FakeXhr[]} */
  const requests = [];
  const answer = () => (up ? { status, body: status === 200 ? STATE : "" } : null);
  const transport = new XhrTransport(fakeRequests(answer, requests), timers);
  const client = new DaemonClient({ port: 47823, send: transport.send, readToken: async () => "t" });
  const session = new SettingsSession(client, timers, {
    platform: "linux",
    readStopRecord: async () => fixtureText("stop-record.json"),
  });
  await session.refresh();
  return { session, timers, requests };
}

/** Fires every timer but the request timeouts' and lets the answers arrive. */
async function fireBurst(/** @type {FakeTimers} */ timers) {
  for (const [id, timer] of [...timers.pending]) {
    if (timer.ms === 600) {
      timers.pending.delete(id);
      timer.callback();
    }
  }
  for (let round = 0; round < 5; round += 1) {
    await settle();
  }
}

test("the lowered session loads the fixture and saves a burst as JSON", async () => {
  const { session, timers, requests } = await widgetSession();
  const scopes = session.setting("linux.scopes");

  assert.equal(session.status, "ready");
  assert.equal(scopes?.value, true);
  /** @type {{ set(on: boolean): void }} */ (/** @type {unknown} */ (scopes)).set(false);
  await fireBurst(timers);

  const patch = requests[1]?.sent;
  assert.equal(patch?.method, "PATCH");
  assert.equal(patch?.url, "http://127.0.0.1:47823/settings");
  assert.equal(patch?.headers["Content-Type"], "application/json");
  assert.deepEqual(JSON.parse(patch?.body ?? ""), {
    revision: "69fd99abe4f654c7",
    changes: [{ kind: "set_switch", key: "linux.scopes", value: false }],
  });
});

test("the lowered session reads a 404 from the settings routes as an outdated service", async () => {
  const { session } = await widgetSession(true, 404);

  assert.equal(session.status, "outdated");
  assert.equal(session.detail, "the service answered 404");
  assert.equal(session.readOnly, true);
  assert.deepEqual(
    settingsBanners(session, "admission", { kind: "port", port: 47823 }, String)[0]?.actions,
    ["restart_service", "open_logs"],
  );
});

test("the lowered session tells a stop from the record when nothing answers", async () => {
  const { session } = await widgetSession(false);

  assert.equal(session.status, "stopped");
  assert.equal(session.stopRecord?.by, "panel");
});
