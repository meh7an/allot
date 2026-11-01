import Gio from "gi://Gio";
import GLib from "gi://GLib";

import { Extension } from "resource:///org/gnome/shell/extensions/extension.js";
import { formatTime } from "resource:///org/gnome/shell/misc/dateUtils.js";
import * as Main from "resource:///org/gnome/shell/ui/main.js";
import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";

import {
  SoupTransport,
  glibTimers,
  readTextFile,
  serviceSystem,
  writeTextFile,
} from "./daemon-io.js";
import { AllotIndicator } from "./indicator.js";
import { DaemonClient } from "./lib/client.js";
import { DEFAULT_PORT, dataPaths, portFromToml } from "./lib/config.js";
import { NotificationRules } from "./lib/notification-rules.js";
import { DaemonPoller } from "./lib/poller.js";
import { PopupModel } from "./lib/popup-model.js";
import { SystemdServiceLauncher } from "./lib/service-launcher.js";
import { serviceMenuItem, stopConfirmation } from "./lib/service-menu.js";
import { decodeSettings, encodeSettings } from "./lib/settings.js";
import { PanelContextMenu } from "./menus.js";
import { Notifier } from "./notifier.js";
import { AllotPopup } from "./popup.js";
import { confirmStop } from "./stop-dialog.js";

/** @import { PollResult } from "./lib/client.js" */
/** @import { NotificationButton } from "./lib/notification-text.js" */
/** @import { ModelChange } from "./lib/popup-model.js" */

// Times on the cards tick once a second while the popup is open.
const TICK_MS = 1000;
const NOTIFICATION_SCHEMA = "org.gnome.desktop.notifications";
const SHOW_BANNERS = "show-banners";

export default class AllotExtension extends Extension {
  enable() {
    this._tickId = 0;
    this._firstOpenWait = null;
    const paths = dataPaths(GLib.get_home_dir());
    const iconDirectory = this.dir.get_child("icons");
    const monotonicMs = () => GLib.get_monotonic_time() / 1000;
    // The monotonic clock, so a wall clock change never stretches a countdown.
    this._model = new PopupModel(monotonicMs);
    this._model.onChange = (change) => this._showChange(change);
    this._model.onStopAnswered = () => this._poller?.pollNow();
    this._model.timeOfDay = (ms) => formatTime(new Date(ms), { timeOnly: true });
    this._model.onSettingsChanged = (settings) => {
      writeTextFile(paths.settings, encodeSettings(settings)).catch((error) => {
        console.warn(`Allot: cannot save ${paths.settings}: ${error.message}`);
      });
    };
    this._rules = new NotificationRules(monotonicMs);
    this._model.onKillSent = (lease) => this._rules?.noteLocalKill(lease);
    this._transport = new SoupTransport();
    this._indicator = new AllotIndicator(
      iconDirectory,
      () => this._popup?.toggle(),
      () => this._openContextMenu(),
    );
    this._popup = new AllotPopup({
      model: this._model,
      indicator: this._indicator,
      iconDirectory,
      logsPath: paths.logs,
      onOpenChanged: (open) => {
        this._poller?.setPopupOpen(open);
        this._scheduleTick();
      },
      onChanged: () => this._showChange({ snapshot: true, indicator: true }),
      onStartService: () => this._startService(),
    });
    this._notifier = new Notifier({
      onButton: (button, target) => this._pressNotificationButton(button, target),
      onActivate: (target) => this._showFromNotification(target),
      currentIcon: () => this._indicator.currentIcon,
    });
    this._contextMenu = new PanelContextMenu({
      source: this._indicator,
      onService: () => this._pressServiceItem(),
      onOpenLogs: () => this._popup?.openLogs(),
    });
    this._contextManager = new PopupMenu.PopupMenuManager(this._indicator);
    Main.uiGroup.add_child(this._contextMenu.menu.actor);
    this._contextMenu.menu.actor.hide();
    this._contextManager.addMenu(this._contextMenu.menu);

    this._notificationSettings = new Gio.Settings({ schema_id: NOTIFICATION_SCHEMA });
    this._notificationSettingsId = this._notificationSettings.connect(
      `changed::${SHOW_BANNERS}`,
      () => this._readDoNotDisturb(),
    );
    this._readDoNotDisturb();

    this._indicator.showStatus(this._model.indicator);
    Main.panel.addToStatusArea(this.uuid, this._indicator);
    this._lastStatus = null;
    this._startPolling(paths, this._transport);
  }

  disable() {
    this._clearFirstOpenWait();
    this._poller?.stop();
    this._transport?.cancel();
    this._notifier?.destroy();
    this._notificationSettings?.disconnect(this._notificationSettingsId);
    this._contextMenu?.menu.destroy();
    this._popup?.destroy();
    this._indicator?.destroy();
    // Closing the popup cancels its confirms, which schedules a tick.
    this._clearTick();
    this._poller = null;
    this._transport = null;
    this._notifier = null;
    this._notificationSettings = null;
    this._contextMenu = null;
    this._contextManager = null;
    this._popup = null;
    this._indicator = null;
    this._rules = null;
    this._model = null;
  }

  /**
   * Reads the settings and the port once, as the other apps do at launch, then polls until
   * `disable`.
   *
   * @param {import("./lib/config.js").DataPaths} paths
   * @param {SoupTransport} transport
   */
  async _startPolling(paths, transport) {
    const [toml, settings] = await Promise.all([
      readTextFile(paths.config),
      readTextFile(paths.settings),
    ]);
    // A disable, or a disable and enable, while the files were being read.
    if (this._transport !== transport) {
      return;
    }
    this._showChange(this._model.loadSettings(decodeSettings(settings)));
    this._popup.syncMenu();
    const client = new DaemonClient({
      port: toml === null ? DEFAULT_PORT : portFromToml(toml),
      send: transport.send,
      readToken: () => readTextFile(paths.token),
    });
    this._model.stopper = client;
    const launcher = new SystemdServiceLauncher(
      {
        home: GLib.get_home_dir(),
        configHome: GLib.getenv("XDG_CONFIG_HOME"),
        root: paths.root,
      },
      serviceSystem,
    );
    const poller = new DaemonPoller(client, glibTimers, {
      launcher,
      now: () => GLib.get_monotonic_time() / 1000,
    });
    poller.onUpdate = (result) => this._show(result);
    poller.setPopupOpen(this._popup?.isOpen ?? false);
    this._poller = poller;
    try {
      await poller.start();
    } catch (error) {
      console.error(`Allot: polling stopped: ${error}`);
    }
  }

  /** @param {PollResult} result */
  _show(result) {
    if (result.status !== this._lastStatus) {
      this._lastStatus = result.status;
      const detail = result.version ?? result.detail ?? "";
      console.log(`Allot: service ${result.status}${detail === "" ? "" : `: ${detail}`}`);
    }
    const lastKnown = this._poller?.lastKnown ?? null;
    this._showChange(this._model.apply(result, lastKnown, this._poller?.startProblem ?? null));
    this._notify(result, lastKnown);
    if (this._model.wantsFirstOpen && this._firstOpenWait === null) {
      this._openForFirstRun();
    }
  }

  /**
   * Opens the popup once the shell can place it: a first poll can answer during the startup
   * animation, or before the new panel button has a position, which only a painted frame
   * promises.
   */
  _openForFirstRun() {
    if (Main.layoutManager._startingUp) {
      this._waitForFirstOpen(Main.layoutManager, "startup-complete", () =>
        this._openForFirstRun(),
      );
      return;
    }
    this._indicator.queue_redraw();
    this._waitForFirstOpen(global.stage, "after-paint", () => {
      this._model.noteFirstOpened();
      if (!this._popup.isOpen) {
        this._popup.open();
      }
    });
  }

  /**
   * @param {{ connect: Function, disconnect: Function }} source
   * @param {string} signal
   * @param {() => void} then Called once, the first time the signal fires.
   */
  _waitForFirstOpen(source, signal, then) {
    this._firstOpenWait = {
      source,
      id: source.connect(signal, () => {
        this._clearFirstOpenWait();
        then();
      }),
    };
  }

  _clearFirstOpenWait() {
    if (this._firstOpenWait !== null) {
      this._firstOpenWait.source.disconnect(this._firstOpenWait.id);
      this._firstOpenWait = null;
    }
  }

  /**
   * @param {PollResult} result
   * @param {PollResult["state"]} lastKnown
   */
  _notify(result, lastKnown) {
    const update = this._rules.observe({
      status: result.status,
      live: result.state,
      lastKnown,
      toggles: this._model.settings.notifications,
      popupOpen: this._popup.isOpen,
    });
    for (const message of update.show) {
      this._notifier.show(message);
    }
    for (const tag of update.withdraw) {
      this._notifier.withdraw(tag);
    }
  }

  /** @param {ModelChange} change */
  _showChange(change) {
    if (change.indicator) {
      this._indicator.showStatus(this._model.indicator);
    }
    if (change.snapshot && this._popup.isOpen) {
      this._popup.render();
    }
    if (this._contextMenu?.menu.isOpen) {
      this._syncContextMenu();
    }
    this._scheduleTick();
  }

  _readDoNotDisturb() {
    const shows = this._notificationSettings.get_boolean(SHOW_BANNERS);
    this._showChange(this._model.setDoNotDisturb(!shows));
  }

  _openContextMenu() {
    this._syncContextMenu();
    this._contextMenu.menu.toggle();
  }

  _syncContextMenu() {
    this._contextMenu.sync(
      this._model.snapshot.header.summary,
      serviceMenuItem(this._model.status),
    );
  }

  _pressServiceItem() {
    switch (serviceMenuItem(this._model.status).action) {
      case "start":
        this._startService();
        break;
      case "stop": {
        const question = stopConfirmation(this._poller?.lastKnown ?? null);
        if (question === null) {
          this._stopService();
        } else {
          confirmStop(question, () => this._stopService());
        }
        break;
      }
      case "none":
        break;
    }
  }

  async _startService() {
    const poller = this._poller;
    if (poller === undefined || poller === null) {
      return;
    }
    this._showChange(this._model.noteStarting());
    const problem = await poller.startService();
    if (problem !== null) {
      console.warn(`Allot: cannot start the service: ${problem}`);
    }
  }

  async _stopService() {
    const poller = this._poller;
    if (poller === undefined || poller === null) {
      return;
    }
    this._rules.expectStop();
    try {
      const problem = await poller.stopService();
      if (problem !== null) {
        console.warn(`Allot: cannot stop the service: ${problem}`);
      }
    } catch (error) {
      // Only the extension being disabled cancels the request.
      console.log(`Allot: stop not finished: ${error}`);
    }
  }

  /**
   * @param {NotificationButton} button
   * @param {string | null} target
   */
  _pressNotificationButton(button, target) {
    switch (button) {
      case "start-service":
        this._startService();
        break;
      case "open-logs":
        this._popup?.openLogs();
        break;
      case "show":
        this._showFromNotification(target);
        break;
    }
  }

  /**
   * Opens the popup after the event ends, since the message list that raised it still holds the
   * pointer until then.
   *
   * @param {string | null} target
   */
  _showFromNotification(target) {
    Main.panel.closeCalendar();
    GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
      this._popup?.show(target);
      return GLib.SOURCE_REMOVE;
    });
  }

  /**
   * Wakes up for the next second while the popup is open, and for the next countdown second
   * or hold end either way, since a Removed card still counts for the panel icon.
   */
  _scheduleTick() {
    this._clearTick();
    if (this._model === null) {
      return;
    }
    const stop = this._model.nextStopChangeMs;
    const open = this._popup?.isOpen ?? false;
    const delays = [open ? TICK_MS : null, stop].filter((delay) => delay !== null);
    if (delays.length === 0) {
      return;
    }
    const delay = Math.max(1, Math.ceil(Math.min(...delays)));
    this._tickId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, delay, () => {
      this._tickId = 0;
      this._showChange(this._model.refresh());
      return GLib.SOURCE_REMOVE;
    });
  }

  _clearTick() {
    if (this._tickId !== 0) {
      GLib.source_remove(this._tickId);
      this._tickId = 0;
    }
  }
}
