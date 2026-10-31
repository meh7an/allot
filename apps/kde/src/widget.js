import { DaemonClient } from "../../gnome/allot@mehran.tech/lib/client.js";
import { DEFAULT_PORT, dataPaths, portFromToml } from "../../gnome/allot@mehran.tech/lib/config.js";
import { accessibleName, iconFile } from "../../gnome/allot@mehran.tech/lib/indicator-status.js";
import { DaemonPoller } from "../../gnome/allot@mehran.tech/lib/poller.js";
import { PopupModel } from "../../gnome/allot@mehran.tech/lib/popup-model.js";
import { CommandRunner, XhrTransport, localPath, readTextFile } from "./io.js";

/** @import { PollResult, ServiceStatus } from "../../gnome/allot@mehran.tech/lib/client.js" */
/** @import { DataPaths } from "../../gnome/allot@mehran.tech/lib/config.js" */
/** @import { Timers } from "../../gnome/allot@mehran.tech/lib/poller.js" */
/** @import { ModelChange } from "../../gnome/allot@mehran.tech/lib/popup-model.js" */
/** @import { IdleLine, SessionStrip } from "../../gnome/allot@mehran.tech/lib/snapshot.js" */
/** @import { JobGroupKind } from "../../gnome/allot@mehran.tech/lib/snapshot.js" */
/** @import { XhrFactory } from "./io.js" */

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

/**
 * What the widget needs from its QML item.
 *
 * @typedef {object} WidgetHost
 * @property {string} homeUrl `StandardPaths`' home location.
 * @property {(command: string) => void} connectSource The executable data source's
 *   `connectSource`; its `onNewData` calls `deliver`.
 * @property {XhrFactory} requests
 * @property {Timers} timers
 * @property {(change: ModelChange) => void} onChange Called after a poll changed the model.
 * @property {(line: string) => void} log
 */

/**
 * The widget's state and polling. Plasma's own Keep Open pin is the popup's Stay open: the popup
 * then stays up when another window takes focus. Plasma anchors the popup to the panel, so
 * there is no Pin with dragging.
 */
export class AllotWidget {
  model = new PopupModel();

  /** @type {DataPaths} */
  paths;

  #host;
  #runner;
  #transport;
  /** @type {DaemonPoller | null} */
  #poller = null;
  /** @type {ServiceStatus | null} */
  #lastStatus = null;
  #expanded = false;
  #stopped = false;

  /** @param {WidgetHost} host */
  constructor(host) {
    this.#host = host;
    this.paths = dataPaths(localPath(host.homeUrl));
    this.#runner = new CommandRunner(host.connectSource);
    this.#transport = new XhrTransport(host.requests, host.timers);
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
   * Follows Plasma's `expanded`: opening polls at once and starts without Keep Open.
   *
   * @param {boolean} expanded
   */
  setExpanded(expanded) {
    this.#expanded = expanded;
    if (expanded) {
      this.model.resetForOpen();
    }
    this.#poller?.setPopupOpen(expanded);
  }

  toggleKeepOpen() {
    this.model.toggleStayOpen();
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
    const change = this.model.apply(result, this.#poller?.lastKnown ?? null);
    if (change.snapshot || change.indicator) {
      this.#host.onChange(change);
    }
  }
}

/**
 * A job row's session line: `a1b2c3d4 · app · 1.5 cores · 2.0 GB`, without empty parts.
 *
 * @param {SessionStrip} strip
 * @returns {string}
 */
export function stripText(strip) {
  return [strip.shortId, strip.project, strip.usage].filter((part) => part !== "").join(" · ");
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
