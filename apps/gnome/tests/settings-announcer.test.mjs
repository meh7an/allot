// What the Settings window speaks to screen readers by itself: errors and outcomes.

import assert from "node:assert/strict";
import { test } from "node:test";

import { NumberModel } from "../allot@mehran.tech/lib/setting-values.js";
import {
  SAVED_ANNOUNCEMENT,
  SettingsAnnouncer,
  announcerView,
} from "../allot@mehran.tech/lib/settings-announcer.js";
import { settingsBanners } from "../allot@mehran.tech/lib/settings-banners.js";
import { as, english, failure, fire, loaded, settleAll } from "./settings-support.mjs";

/**
 * @import { AnnouncerView } from "../allot@mehran.tech/lib/settings-announcer.js"
 * @import { Reach } from "../allot@mehran.tech/lib/service-row.js"
 */

/** @type {Reach} */
const PORT_REACH = { kind: "port", port: 47823 };

/**
 * @param {Partial<AnnouncerView>} view
 * @returns {AnnouncerView}
 */
function view(view) {
  return { banners: [], issues: [], save: "saved", ...view };
}

test("the first view only sets what is on screen", () => {
  const announcer = new SettingsAnnouncer();

  assert.equal(announcer.update(view({ banners: ["The service is stopped."] })), "");
  assert.equal(announcer.update(view({ banners: ["The service is stopped."] })), "");
});

test("a banner is spoken once, when it appears", () => {
  const announcer = new SettingsAnnouncer();
  announcer.update(view({}));

  assert.equal(announcer.update(view({ banners: ["A", "B"] })), "A B");
  assert.equal(announcer.update(view({ banners: ["B"] })), "");
  assert.equal(announcer.update(view({ banners: ["A", "B"] })), "A");
});

test("another page's banners are read with the page, not spoken", () => {
  const announcer = new SettingsAnnouncer();
  announcer.update(view({ banners: ["A"] }));

  announcer.pageShown();

  assert.equal(announcer.update(view({ banners: ["A", "Restart the service."] })), "");
});

test("an error is spoken when it appears or its text changes, not while it stays", () => {
  const announcer = new SettingsAnnouncer();
  const range = { key: "a", text: "Headroom: Enter a number at least 0 and below 100." };
  const blank = { key: "a", text: "Headroom: Enter a value." };

  assert.equal(announcer.update(view({ issues: [range] })), range.text);
  assert.equal(announcer.update(view({ issues: [range] })), "");
  assert.equal(announcer.update(view({ issues: [blank] })), blank.text);
  assert.equal(announcer.update(view({ issues: [] })), "");
  assert.equal(announcer.update(view({ issues: [blank] })), blank.text);
});

test("a save that ends with nothing unsaved says so once", () => {
  const announcer = new SettingsAnnouncer();
  announcer.update(view({}));

  assert.equal(announcer.update(view({ save: "saving" })), "");
  assert.equal(announcer.update(view({ save: "saved" })), SAVED_ANNOUNCEMENT);
  assert.equal(announcer.update(view({ save: "saved" })), "");
});

test("a save that ends in a problem speaks the problem, not saved", () => {
  const announcer = new SettingsAnnouncer();
  announcer.update(view({ save: "saving" }));

  assert.equal(announcer.update(view({ save: "unsaved" })), "");

  announcer.update(view({ save: "saving" }));
  assert.equal(
    announcer.update(view({ save: "saved", banners: ["config.toml changed outside Settings."] })),
    "config.toml changed outside Settings.",
  );
});

test("a session's out-of-range value is spoken with its label, then the save", async () => {
  const { session, timers } = await loaded();
  const announcer = new SettingsAnnouncer();
  const speak = () =>
    announcer.update(
      announcerView(session, settingsBanners(session, "admission", PORT_REACH, english), english),
    );
  speak();
  /** @type {string[]} */
  const spoken = [];
  session.onChange = () => {
    const text = speak();
    if (text !== "") {
      spoken.push(text);
    }
  };
  const headroom = as(session.setting("admission.headroom_pct"), NumberModel);

  headroom.commit(150);
  headroom.commit(20);
  await fire(timers);

  assert.deepEqual(spoken, [
    `${headroom.label}: Enter a number at least 0 and below 100.`,
    SAVED_ANNOUNCEMENT,
  ]);
});

test("allotd going away speaks its banner", async () => {
  const { session, client } = await loaded();
  const announcer = new SettingsAnnouncer();
  const speak = () =>
    announcer.update(
      announcerView(session, settingsBanners(session, "admission", PORT_REACH, english), english),
    );
  speak();

  client.reads.push(failure("unreachable"));
  await session.refresh();
  await settleAll();

  assert.equal(speak(), "The service isn't answering on port 47823. Trying again every 5 s.");
});
