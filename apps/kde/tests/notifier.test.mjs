import assert from "node:assert/strict";
import { test } from "node:test";

import { settle } from "../../gnome/tests/support.mjs";
import { Notifier } from "../src/notifier.js";
import { FakeNotification } from "./support.mjs";

/** @import { NotificationMessage } from "../../gnome/allot@mehran.tech/lib/notification-text.js" */

/**
 * @param {Partial<NotificationMessage>} [fields]
 * @returns {NotificationMessage}
 */
function message(fields = {}) {
  return {
    kind: "job-finished",
    tag: "done-1",
    title: "cargo finished",
    body: "4:52",
    buttons: ["show"],
    target: "recent",
    ...fields,
  };
}

function subject() {
  /** @type {FakeNotification[]} */
  const sent = [];
  /** @type {[string, string | null][]} */
  const pressed = [];
  /** @type {(string | null)[]} */
  const activated = [];
  const notifier = new Notifier({
    send: (fields) => {
      const made = new FakeNotification(fields);
      sent.push(made);
      return made;
    },
    onButton: (button, target) => pressed.push([button, target]),
    onActivate: (target) => activated.push(target),
  });
  return { notifier, sent, pressed, activated };
}

test("a message becomes a notification with its fields and labelled buttons", () => {
  const { notifier, sent, pressed } = subject();

  notifier.show(
    message({ buttons: ["start-service", "open-logs"], target: null }),
    "allot-down-symbolic.svg",
  );

  const [made] = sent;
  assert.equal(made?.fields.title, "cargo finished");
  assert.equal(made?.fields.body, "4:52");
  assert.equal(made?.fields.icon, "allot-down-symbolic.svg");
  assert.deepEqual(
    made?.fields.buttons.map((button) => button.label),
    ["Start Service", "Open Logs"],
  );
  made?.press("Open Logs");
  assert.deepEqual(pressed, [["open-logs", null]]);
});

test("a click activates the message's target", () => {
  const { notifier, sent, activated } = subject();

  notifier.show(message(), "icon");
  sent[0]?.fields.activated();

  assert.deepEqual(activated, ["recent"]);
});

test("the same tag closes the old notification and sends a new one", async () => {
  const { notifier, sent } = subject();

  notifier.show(message(), "icon");
  notifier.show(message({ title: "2 jobs finished" }), "icon");
  await settle();

  assert.equal(sent.length, 2);
  assert.equal(sent[0]?.closedByWidget, true);
  assert.equal(sent[1]?.closedByWidget, false);
  assert.equal(sent[1]?.fields.title, "2 jobs finished");

  // The old one's late close must not forget the new one.
  notifier.withdraw("done-1");
  assert.equal(sent[1]?.closedByWidget, true);
});

test("a closed notification is forgotten, so withdrawing it does nothing", () => {
  const { notifier, sent } = subject();

  notifier.show(message(), "icon");
  sent[0]?.fields.closed();
  notifier.withdraw("done-1");

  assert.equal(sent[0]?.closedByWidget, false);
});

test("withdrawing an unknown tag does nothing", () => {
  const { notifier } = subject();

  notifier.withdraw("service");
});
