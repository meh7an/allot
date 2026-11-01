import { Announcer } from "../../gnome/allot@mehran.tech/lib/accessible-text.js";
import { DaemonClient } from "../../gnome/allot@mehran.tech/lib/client.js";
import { DEFAULT_PORT, dataPaths, portFromToml } from "../../gnome/allot@mehran.tech/lib/config.js";
import { accessibleName, iconFile } from "../../gnome/allot@mehran.tech/lib/indicator-status.js";
import { meterCount, notificationCount } from "../../gnome/allot@mehran.tech/lib/menu-counts.js";
import { NotificationRules } from "../../gnome/allot@mehran.tech/lib/notification-rules.js";
import { DaemonPoller } from "../../gnome/allot@mehran.tech/lib/poller.js";
import { PopupModel } from "../../gnome/allot@mehran.tech/lib/popup-model.js";
import { SystemdServiceLauncher } from "../../gnome/allot@mehran.tech/lib/service-launcher.js";
import {
  STOP_CONFIRM_TITLE,
  serviceMenuItem,
  stopConfirmation,
} from "../../gnome/allot@mehran.tech/lib/service-menu.js";
import { decodeSettings, encodeSettings } from "../../gnome/allot@mehran.tech/lib/settings.js";
import {
  DO_NOT_DISTURB_TEXT,
  WELCOME_BODY,
  WELCOME_TITLE,
} from "../../gnome/allot@mehran.tech/lib/snapshot.js";
import { AppIcons } from "./app-icons.js";
import {
  CommandRunner,
  XhrTransport,
  copyFile,
  detachedCommand,
  localPath,
  readTextFile,
  serviceSystem,
  writeTextFile,
} from "./io.js";
import {
  INHIBITED_COMMAND,
  NOTIFY_COMPONENT,
  NOTIFY_EVENT,
  NOTIFY_RC_FILE,
  SETTINGS_ARGV,
  isOffInSettings,
  parseInhibited,
} from "./notification-state.js";
import { Notifier } from "./notifier.js";
import { RowKeeper, listRows } from "./rows.js";

/** @import { PollResult, ServiceStatus } from "../../gnome/allot@mehran.tech/lib/client.js" */
/** @import { DataPaths } from "../../gnome/allot@mehran.tech/lib/config.js" */
/** @import { Timers } from "../../gnome/allot@mehran.tech/lib/poller.js" */
/** @import { NotificationButton } from "../../gnome/allot@mehran.tech/lib/notification-text.js" */
/** @import { ModelChange } from "../../gnome/allot@mehran.tech/lib/popup-model.js" */
/** @import { ServiceMenuItem } from "../../gnome/allot@mehran.tech/lib/service-menu.js" */
/** @import { MeterToggles } from "../../gnome/allot@mehran.tech/lib/settings.js" */
/** @import { NotificationToggles } from "../../gnome/allot@mehran.tech/lib/settings.js" */
/** @import { IdleLine } from "../../gnome/allot@mehran.tech/lib/snapshot.js" */
/** @import { JobGroupKind } from "../../gnome/allot@mehran.tech/lib/snapshot.js" */
/** @import { XhrFactory } from "./io.js" */
/** @import { NotificationFields, SentNotification } from "./notifier.js" */
/** @import { Row, RowModel } from "./rows.js" */

export {
  DO_NOT_DISTURB_TEXT,
  NOTIFY_COMPONENT,
  NOTIFY_EVENT,
  STOP_CONFIRM_TITLE,
  WELCOME_BODY,
  WELCOME_TITLE,
};

/** Breeze icons for the group headings, the handoff's timer, play and clock. */
export const GROUP_ICONS = {
  benchmark: "chronometer",
  running: "media-playback-start",
  waiting: "clock",
};

export const NOTHING_TEXT = "No jobs running or waiting.";

/** The opacity of the idle header glyph and of the last known jobs while allotd is away. */
export const DIMMED = 0.5;

/** The header glyph while the service is not serving state. */
export const ERROR_ICON = "dialog-error";

/** The header glyph otherwise, under the package's `icons` folder; dimmed while idle. */
export const MARK_ICON_FILE = "allot-mark-symbolic.svg";

/** The Notifications submenu's first line while System Settings has them off. */
export const NOTIFICATIONS_OFF_TEXT = "Turned off in System Settings";

/** The first-run card's line for the same. */
export const NOTIFICATIONS_OFF_CARD_TEXT = "Notifications are turned off in System Settings.";

/** The menu's count while System Settings has them off. */
export const NOTIFICATIONS_OFF_COUNT = "Off";

// Times on the cards tick once a second while the popup is open.
const TICK_MS = 1000;

/**
 * What the widget needs from its QML item.
 *
 * @typedef {object} WidgetHost
 * @property {string} homeUrl `StandardPaths`' home location.
 * @property {string} configUrl `StandardPaths`' config location.
 * @property {string} dataUrl `StandardPaths`' generic data location.
 * @property {string} notifyRcUrl The notifyrc in the package.
 * @property {(command: string) => void} connectSource The executable data source's
 *   `connectSource`; its `onNewData` calls `deliver`.
 * @property {XhrFactory} requests
 * @property {Timers} timers
 * @property {RowModel} rows The ListModel the popup's list shows.
 * @property {(data: Record<string, Row>, animate: boolean) => void} showRows
 *   Hands the views their rows, by key; `animate` is off when the popup opens.
 * @property {(change: ModelChange) => void} onChange Called after the model changed.
 * @property {(fields: NotificationFields) => SentNotification} sendNotification
 * @property {() => void} expand Opens the popup.
 * @property {(key: string | null) => void} scrollTo Scrolls the open popup to a row, or the top.
 * @property {(url: string) => void} openUrl
 * @property {(text: string) => void} announce Says something to a screen reader.
 * @property {(line: string) => void} log
 * @property {(ms: number) => string} [timeOfDay] A daemon time as the user's short time of day.
 * @property {() => number} [now] The clock that ticks times and stops; the tests pass their own.
 */

/**
 * The widget's state and polling. Plasma's own Keep Open pin is the popup's Stay open: the popup
 * then stays up when another window takes focus. Plasma anchors the popup to the panel, so
 * there is no Pin with dragging.
 */
export class AllotWidget {
  /** @type {PopupModel} */
  model;

  /** @type {DataPaths} */
  paths;

  /**
   * The question the popup's Stop card asks, or empty.
   *
   * @type {string}
   */
  stopQuestion = "";

  /**
   * Allot's notifications are turned off in System Settings.
   *
   * @type {boolean}
   */
  notificationsOff = false;

  #host;
  #runner;
  #transport;
  #keeper;
  #appIcons;
  #rules;
  #notifier;
  #configFolder;
  #dataFolder;
  #announcer = new Announcer();
  /** @type {DaemonPoller | null} */
  #poller = null;
  /** @type {ServiceStatus | null} */
  #lastStatus = null;
  /** @type {unknown} */
  #tick = null;
  /** @type {Promise<void>} */
  #saving = Promise.resolve();
  #expanded = false;
  #stopped = false;

  /** @param {WidgetHost} host */
  constructor(host) {
    this.#host = host;
    this.model = new PopupModel(host.now);
    this.model.onChange = (change) => this.#showChange(change);
    this.model.onStopAnswered = () => this.#poller?.pollNow();
    this.model.onKillSent = (lease) => this.#rules.noteLocalKill(lease);
    this.model.onSettingsChanged = () => this.#saveSettings();
    if (host.timeOfDay !== undefined) {
      this.model.timeOfDay = host.timeOfDay;
    }
    this.paths = dataPaths(localPath(host.homeUrl));
    this.#configFolder = localPath(host.configUrl);
    this.#dataFolder = localPath(host.dataUrl);
    this.#runner = new CommandRunner(host.connectSource);
    this.#transport = new XhrTransport(host.requests, host.timers);
    this.#keeper = new RowKeeper(host.rows);
    this.#appIcons = new AppIcons(this.#runner, () => {
      this.#showChange({ snapshot: true, indicator: false });
    });
    this.#rules = new NotificationRules(host.now);
    this.#notifier = new Notifier({
      send: host.sendNotification,
      onButton: (button, target) => this.#pressNotificationButton(button, target),
      onActivate: (target) => this.#showRow(target),
    });
  }

  /**
   * Reads the settings and the port once, as the other apps do at launch, puts the notifyrc where
   * Plasma looks for it, then polls until `stop`. A widget's package cannot install that file.
   *
   * @returns {Promise<void>}
   */
  async start() {
    const [toml, settings, notifyRc] = await Promise.all([
      readTextFile(this.#runner, this.paths.config),
      readTextFile(this.#runner, this.paths.settings),
      copyFile(
        this.#runner,
        localPath(this.#host.notifyRcUrl),
        `${this.#dataFolder}/knotifications6/${NOTIFY_RC_FILE}`,
      ),
    ]);
    if (this.#stopped) {
      return;
    }
    if (!notifyRc) {
      this.#host.log("Allot: cannot install the notifyrc; notifications will not show");
    }
    this.#showChange(this.model.loadSettings(decodeSettings(settings)));
    this.refreshNotificationState();
    const client = new DaemonClient({
      port: toml === null ? DEFAULT_PORT : portFromToml(toml),
      send: this.#transport.send,
      readToken: () => readTextFile(this.#runner, this.paths.token),
    });
    this.model.stopper = client;
    const launcher = new SystemdServiceLauncher(
      {
        home: localPath(this.#host.homeUrl),
        configHome: this.#configFolder,
        root: this.paths.root,
      },
      serviceSystem(this.#runner),
    );
    const poller = new DaemonPoller(client, this.#host.timers, {
      launcher,
      ...(this.#host.now === undefined ? {} : { now: this.#host.now }),
    });
    poller.onUpdate = (result) => this.#show(result);
    poller.setPopupOpen(this.#expanded);
    this.#poller = poller;
    try {
      await poller.start();
    } catch (error) {
      this.#host.log(`Allot: polling stopped: ${error}`);
    }
  }

  stop() {
    this.#stopped = true;
    this.#clearTick();
    this.#poller?.stop();
    this.#transport.cancel();
  }

  /**
   * The executable data source's answers.
   *
   * @param {string} command
   * @param {Record<string, unknown>} data
   */
  deliver(command, data) {
    this.#runner.deliver(command, data);
  }

  /**
   * Follows Plasma's `expanded`: opening polls at once, starts without Keep Open and shows the
   * rows without animating them; closing takes back any Kill? and the Stop card.
   *
   * @param {boolean} expanded
   */
  setExpanded(expanded) {
    this.#expanded = expanded;
    if (expanded) {
      this.model.resetForOpen();
      this.model.refresh();
      this.#announcer.reset();
      this.#render(false);
      this.refreshNotificationState();
    } else {
      this.model.cancelConfirms();
      this.#appIcons.forget();
      this.stopQuestion = "";
    }
    this.#poller?.setPopupOpen(expanded);
    this.#scheduleTick();
  }

  toggleKeepOpen() {
    this.model.toggleStayOpen();
  }

  /**
   * A press of Kill, or of Kill? within its countdown.
   *
   * @param {number} lease
   */
  pressKill(lease) {
    this.model.pressKill(lease);
    this.#showPress();
  }

  /**
   * A press of Remove on a queued request.
   *
   * @param {number} lease
   */
  pressRemove(lease) {
    this.model.pressRemove(lease);
    this.#showPress();
  }

  /**
   * Focus left a Kill? confirm.
   *
   * @param {number} lease
   */
  cancelConfirm(lease) {
    if (this.model.cancelConfirm(lease)) {
      this.#showPress();
    }
  }

  /** Esc takes back a Kill? confirm before it closes the popup. */
  cancelConfirms() {
    if (this.model.cancelConfirms()) {
      this.#showPress();
    }
  }

  /**
   * A leaving row's view has collapsed.
   *
   * @param {string} key
   */
  rowGone(key) {
    this.#keeper.gone(key);
  }

  /** The Resources card's header. */
  toggleResources() {
    this.model.toggleResources();
    this.#showPress();
  }

  /**
   * A Meters check item.
   *
   * @param {keyof MeterToggles} key
   */
  toggleMeter(key) {
    this.model.updateSettings((settings) => {
      settings.meters[key] = !settings.meters[key];
    });
    this.#showPress();
  }

  /**
   * A Notifications check item.
   *
   * @param {keyof NotificationToggles} key
   */
  toggleNotification(key) {
    this.model.updateSettings((settings) => {
      settings.notifications[key] = !settings.notifications[key];
    });
    this.#showPress();
  }

  /** The first-run card's Done. */
  finishFirstRun() {
    this.model.finishFirstRun();
    this.#showPress();
  }

  /** The banner's Start Service, or the panel menu's while allotd is down. */
  async startService() {
    const poller = this.#poller;
    if (poller === null) {
      return;
    }
    this.#showChange(this.model.noteStarting());
    const problem = await poller.startService();
    if (problem !== null) {
      this.#host.log(`Allot: cannot start the service: ${problem}`);
    }
  }

  /** The panel menu's service item: with jobs up, Stop asks in the popup first. */
  pressServiceItem() {
    switch (serviceMenuItem(this.model.status).action) {
      case "start":
        this.startService();
        break;
      case "stop": {
        const question = stopConfirmation(this.#poller?.lastKnown ?? null);
        if (question === null) {
          this.#stopService();
        } else {
          this.stopQuestion = question;
          this.#host.expand();
          this.#host.onChange({ snapshot: false, indicator: false });
        }
        break;
      }
      case "none":
        break;
    }
  }

  /** The Stop card's Stop. */
  confirmStop() {
    this.cancelStop();
    this.#stopService();
  }

  /** The Stop card's Cancel. */
  cancelStop() {
    this.stopQuestion = "";
    this.#host.onChange({ snapshot: false, indicator: false });
  }

  /**
   * Reads whether Plasma would show notifications now: when the popup or More options opens, since
   * Plasma tells the widget of neither switch.
   */
  async refreshNotificationState() {
    const [inhibited, events, plasma] = await Promise.all([
      this.#runner.run(INHIBITED_COMMAND),
      readTextFile(this.#runner, `${this.#configFolder}/${NOTIFY_RC_FILE}`),
      readTextFile(this.#runner, `${this.#configFolder}/plasmanotifyrc`),
    ]);
    if (this.#stopped) {
      return;
    }
    const off = isOffInSettings(events, plasma);
    const doNotDisturb = inhibited.exitCode === 0 && parseInhibited(inhibited.stdout);
    if (off !== this.notificationsOff || doNotDisturb !== this.model.doNotDisturb) {
      this.notificationsOff = off;
      this.model.setDoNotDisturb(doNotDisturb);
      this.#showPress();
    }
  }

  /** The menu's and the first-run card's way to turn notifications back on. */
  openNotificationSettings() {
    this.#runner.run(detachedCommand(SETTINGS_ARGV));
  }

  /** Opens the logs folder. */
  openLogs() {
    this.#host.openUrl(`file://${this.paths.logs}`);
  }

  /** @returns {boolean} */
  get keepOpen() {
    return this.model.isStayOpen;
  }

  /** @returns {string} The panel icon's file under the package's `icons` folder. */
  get iconFile() {
    return iconFile(this.model.indicator.icon);
  }

  /** @returns {string} */
  get accessibleName() {
    return accessibleName(this.model.indicator);
  }

  /** @returns {ServiceMenuItem} */
  get serviceItem() {
    return serviceMenuItem(this.model.status);
  }

  /** @returns {string} */
  get meterCount() {
    return meterCount(this.model.settings.meters);
  }

  /** @returns {string} */
  get notificationCount() {
    return this.notificationsOff
      ? NOTIFICATIONS_OFF_COUNT
      : notificationCount(this.model.settings.notifications);
  }

  /** @param {PollResult} result */
  #show(result) {
    if (result.status !== this.#lastStatus) {
      this.#lastStatus = result.status;
      const detail = result.version ?? result.detail ?? "";
      this.#host.log(`Allot: service ${result.status}${detail === "" ? "" : `: ${detail}`}`);
    }
    const lastKnown = this.#poller?.lastKnown ?? null;
    const startProblem = this.#poller?.startProblem ?? null;
    this.#showChange(this.model.apply(result, lastKnown, startProblem));
    this.#notify(result);
    if (this.model.wantsFirstOpen) {
      this.model.noteFirstOpened();
      this.#host.expand();
    }
  }

  /** @param {PollResult} result */
  #notify(result) {
    const update = this.#rules.observe({
      status: result.status,
      live: result.state,
      lastKnown: this.#poller?.lastKnown ?? null,
      toggles: this.model.settings.notifications,
      popupOpen: this.#expanded,
    });
    for (const message of update.show) {
      this.#notifier.show(message, this.iconFile);
      this.#host.log(`Allot: notified ${message.tag}`);
    }
    for (const tag of update.withdraw) {
      this.#notifier.withdraw(tag);
    }
  }

  /**
   * @param {NotificationButton} button
   * @param {string | null} target
   */
  #pressNotificationButton(button, target) {
    switch (button) {
      case "start-service":
        this.startService();
        break;
      case "open-logs":
        this.openLogs();
        break;
      case "show":
        this.#showRow(target);
        break;
    }
  }

  /** @param {string | null} target */
  #showRow(target) {
    this.#host.expand();
    this.#host.scrollTo(target);
  }

  async #stopService() {
    const poller = this.#poller;
    if (poller === null) {
      return;
    }
    this.#rules.expectStop();
    try {
      const problem = await poller.stopService();
      if (problem !== null) {
        this.#host.log(`Allot: cannot stop the service: ${problem}`);
      }
    } catch (error) {
      // Only the widget going away cancels the request.
      this.#host.log(`Allot: stop not finished: ${error}`);
    }
  }

  // One write at a time, in order, so a slow one never replaces a newer one.
  #saveSettings() {
    const text = encodeSettings(this.model.settings);
    this.#saving = this.#saving.then(async () => {
      if (!(await writeTextFile(this.#runner, this.paths.settings, text))) {
        this.#host.log(`Allot: cannot save ${this.paths.settings}`);
      }
    });
  }

  #showPress() {
    this.#showChange({ snapshot: true, indicator: true });
  }

  /** @param {ModelChange} change */
  #showChange(change) {
    if (change.snapshot && this.#expanded) {
      this.#render(true);
    }
    if (change.snapshot || change.indicator) {
      this.#host.onChange(change);
    }
    this.#scheduleTick();
  }

  /** @param {boolean} animate */
  #render(animate) {
    const { snapshot } = this.model;
    const text = {
      idle: snapshot.idle === null ? "" : idleText(snapshot.idle),
      nothing: NOTHING_TEXT,
    };
    const rows = listRows(snapshot, this.#appIcons.icon, text, this.notificationsOff);
    this.#keeper.show(rows, animate, (data) => this.#host.showRows(data, animate));
    for (const spoken of this.#announcer.observe(snapshot)) {
      this.#host.announce(spoken);
    }
  }

  /**
   * Wakes up for the next second while the popup is open, and for the next countdown second or
   * hold end either way, since a Removed card still counts for the panel icon.
   */
  #scheduleTick() {
    this.#clearTick();
    if (this.#stopped) {
      return;
    }
    const stop = this.model.nextStopChangeMs;
    const delays = [];
    if (this.#expanded) {
      delays.push(TICK_MS);
    }
    if (stop !== null) {
      delays.push(stop);
    }
    if (delays.length === 0) {
      return;
    }
    this.#tick = this.#host.timers.setTimeout(
      () => {
        this.#tick = null;
        this.#showChange(this.model.refresh());
      },
      Math.max(1, Math.ceil(Math.min(...delays))),
    );
  }

  #clearTick() {
    if (this.#tick !== null) {
      this.#host.timers.clearTimeout(this.#tick);
      this.#tick = null;
    }
  }
}

/**
 * @param {IdleLine} idle
 * @returns {string}
 */
export function idleText(idle) {
  return [idle.label, idle.ids, idle.usage].filter((part) => part !== "").join("  ");
}

/**
 * @param {JobGroupKind} kind
 * @returns {string}
 */
export function groupIcon(kind) {
  return GROUP_ICONS[kind];
}
