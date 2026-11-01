import { DaemonClient } from "../../gnome/allot@mehran.tech/lib/client.js";
import { DEFAULT_PORT, dataPaths, portFromToml } from "../../gnome/allot@mehran.tech/lib/config.js";
import { accessibleName, iconFile } from "../../gnome/allot@mehran.tech/lib/indicator-status.js";
import { DaemonPoller } from "../../gnome/allot@mehran.tech/lib/poller.js";
import { PopupModel } from "../../gnome/allot@mehran.tech/lib/popup-model.js";
import { AppIcons } from "./app-icons.js";
import { CommandRunner, XhrTransport, localPath, readTextFile } from "./io.js";
import { RowKeeper, listRows } from "./rows.js";

/** @import { PollResult, ServiceStatus } from "../../gnome/allot@mehran.tech/lib/client.js" */
/** @import { DataPaths } from "../../gnome/allot@mehran.tech/lib/config.js" */
/** @import { Timers } from "../../gnome/allot@mehran.tech/lib/poller.js" */
/** @import { ModelChange } from "../../gnome/allot@mehran.tech/lib/popup-model.js" */
/** @import { IdleLine } from "../../gnome/allot@mehran.tech/lib/snapshot.js" */
/** @import { JobGroupKind } from "../../gnome/allot@mehran.tech/lib/snapshot.js" */
/** @import { XhrFactory } from "./io.js" */
/** @import { Row, RowModel } from "./rows.js" */

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

// Times on the cards tick once a second while the popup is open.
const TICK_MS = 1000;

/**
 * What the widget needs from its QML item.
 *
 * @typedef {object} WidgetHost
 * @property {string} homeUrl `StandardPaths`' home location.
 * @property {(command: string) => void} connectSource The executable data source's
 *   `connectSource`; its `onNewData` calls `deliver`.
 * @property {XhrFactory} requests
 * @property {Timers} timers
 * @property {RowModel} rows The ListModel the popup's list shows.
 * @property {(data: Record<string, Row>, animate: boolean) => void} showRows
 *   Hands the views their rows, by key; `animate` is off when the popup opens.
 * @property {(change: ModelChange) => void} onChange Called after the model changed.
 * @property {(line: string) => void} log
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

  #host;
  #runner;
  #transport;
  #keeper;
  #appIcons;
  /** @type {DaemonPoller | null} */
  #poller = null;
  /** @type {ServiceStatus | null} */
  #lastStatus = null;
  /** @type {unknown} */
  #tick = null;
  #expanded = false;
  #stopped = false;

  /** @param {WidgetHost} host */
  constructor(host) {
    this.#host = host;
    this.model = new PopupModel(host.now);
    this.model.onChange = (change) => this.#showChange(change);
    this.model.onStopAnswered = () => this.#poller?.pollNow();
    this.paths = dataPaths(localPath(host.homeUrl));
    this.#runner = new CommandRunner(host.connectSource);
    this.#transport = new XhrTransport(host.requests, host.timers);
    this.#keeper = new RowKeeper(host.rows);
    this.#appIcons = new AppIcons(this.#runner, () => {
      this.#showChange({ snapshot: true, indicator: false });
    });
  }

  /**
   * Reads the port once, as the other apps do at launch, then polls until `stop`.
   *
   * @returns {Promise<void>}
   */
  async start() {
    const toml = await readTextFile(this.#runner, this.paths.config);
    if (this.#stopped) {
      return;
    }
    const client = new DaemonClient({
      port: toml === null ? DEFAULT_PORT : portFromToml(toml),
      send: this.#transport.send,
      readToken: () => readTextFile(this.#runner, this.paths.token),
    });
    this.model.stopper = client;
    const poller = new DaemonPoller(client, this.#host.timers);
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
   * rows without animating them; closing takes back any Kill?.
   *
   * @param {boolean} expanded
   */
  setExpanded(expanded) {
    this.#expanded = expanded;
    if (expanded) {
      this.model.resetForOpen();
      this.model.refresh();
      this.#render(false);
    } else {
      this.model.cancelConfirms();
      this.#appIcons.forget();
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

  /** @param {PollResult} result */
  #show(result) {
    if (result.status !== this.#lastStatus) {
      this.#lastStatus = result.status;
      const detail = result.version ?? result.detail ?? "";
      this.#host.log(`Allot: service ${result.status}${detail === "" ? "" : `: ${detail}`}`);
    }
    this.#showChange(this.model.apply(result, this.#poller?.lastKnown ?? null));
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
    const rows = listRows(snapshot, this.#appIcons.icon, text);
    this.#keeper.show(rows, animate, (data) => this.#host.showRows(data, animate));
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
