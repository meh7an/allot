import Gio from "gi://Gio";
import GLib from "gi://GLib";

import { Extension } from "resource:///org/gnome/shell/extensions/extension.js";
import { formatTime } from "resource:///org/gnome/shell/misc/dateUtils.js";
import * as Main from "resource:///org/gnome/shell/ui/main.js";
import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";

import {
  DaemonConnection,
  createServiceSystem,
  glibTimers,
  readTextFile,
  writeTextFile,
} from "./daemon-io.js";
import { AllotIndicator } from "./indicator.js";
import { dataPaths } from "./lib/config.js";
import { NotificationRules } from "./lib/notification-rules.js";
import { DaemonPoller } from "./lib/poller.js";
import { PopupModel } from "./lib/popup-model.js";
import { CancelledError } from "./lib/results.js";
import { SystemdServiceLauncher } from "./lib/service-launcher.js";
import { serviceMenuItem, stopConfirmation } from "./lib/service-menu.js";
import { decodeSettings, encodeSettings } from "./lib/settings.js";
import { PanelContextMenu } from "./menus.js";
import { Notifier } from "./notifier.js";
import { AllotPopup } from "./popup.js";
import { confirmStop } from "./stop-dialog.js";

/** @import { PollResult } from "./lib/results.js" */
/** @import { NotificationButton } from "./lib/notification-text.js" */
/** @import { ModelChange } from "./lib/popup-model.js" */

// Times on the cards tick once a second while the popup is open.
const TICK_MS = 1000;
const NOTIFICATION_SCHEMA = "org.gnome.desktop.notifications";
const SHOW_BANNERS = "show-banners";
// The gnome-shell service that runs every extension's prefs.js.
const PREFS_APP_ID = "org.gnome.Shell.Extensions";
const OTHER_PREFS_TEXT =
  "Another extension's settings are open. Close them first, then open Settings again.";
// How long after enable the shell's startup animation is assumed over, when its signal has not
// come. The animation lasts about a second.
const STARTUP_GRACE_MS = 5000;
// Lets a write's burst of monitor events end before the file is read.
const SETTINGS_READ_DELAY_MS = 100;

export default class AllotExtension extends Extension {
  enable() {
    this._tickId = 0;
    this._showIdleId = 0;
    this._firstOpenWait = null;
    this._stopDialog = null;
    this._watchStartup();
    this._files = new Gio.Cancellable();
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
    this._connection = new DaemonConnection();
    this._indicator = new AllotIndicator(
      iconDirectory,
      (byKeyboard) => this._popup?.toggle(byKeyboard),
      () => this._openContextMenu(),
    );
    this._popup = new AllotPopup({
      model: this._model,
      indicator: this._indicator,
      iconDirectory,
      contrastSheet: this.dir.get_child("high-contrast.css"),
      logsPath: paths.logs,
      onOpenChanged: (open) => {
        this._poller?.setPopupOpen(open);
        this._scheduleTick();
      },
      onChanged: () => this._showChange({ snapshot: true, indicator: true }),
      onStartService: () => this._startService(),
      onSettings: () => this._openSettings(),
      onService: () => this._pressServiceItem(),
    });
    this._notifier = new Notifier({
      onButton: (button, target) => this._pressNotificationButton(button, target),
      onActivate: (target) => this._showFromNotification(target),
      currentIcon: () => this._indicator.currentIcon,
    });
    this._contextMenu = new PanelContextMenu({
      source: this._indicator,
      onSettings: () => this._openSettings(),
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
    this._watchSettings(paths);
    this._startPolling(paths, this._connection, this._files);
  }

  disable() {
    this._settingsMonitor?.cancel();
    if (this._settingsReadId) {
      GLib.source_remove(this._settingsReadId);
    }
    this._clearStartupWatch();
    this._clearFirstOpenWait();
    this._clearShowIdle();
    this._stopDialog?.close();
    this._poller?.stop();
    this._connection?.cancel();
    this._files?.cancel();
    this._notifier?.destroy();
    this._notificationSettings?.disconnect(this._notificationSettingsId);
    this._contextMenu?.menu.destroy();
    this._popup?.destroy();
    this._indicator?.destroy();
    // Closing the popup cancels its confirms, which schedules a tick.
    this._clearTick();
    this._poller = null;
    this._connection = null;
    this._files = null;
    this._notifier = null;
    this._notificationSettings = null;
    this._contextMenu = null;
    this._contextManager = null;
    this._popup = null;
    this._indicator = null;
    this._rules = null;
    this._model = null;
    this._settingsMonitor = null;
    this._settingsReadId = 0;
  }

  /**
   * Reads the settings once, as the other apps do at launch, then polls until `disable`. allotd
   * taking or leaving its name, and its JobsChanged, poll at once.
   *
   * @param {import("./lib/config.js").DataPaths} paths
   * @param {DaemonConnection} connection
   * @param {Gio.Cancellable} files Ends the file reads and service waits in flight.
   */
  async _startPolling(paths, connection, files) {
    const settings = await readTextFile(paths.settings, files);
    // A disable, or a disable and enable, while the file was being read.
    if (this._connection !== connection) {
      return;
    }
    this._showChange(this._model.loadSettings(decodeSettings(settings)));
    this._popup.syncMenu();
    const client = connection.client();
    this._model.stopper = client;
    const launcher = new SystemdServiceLauncher(
      {
        home: GLib.get_home_dir(),
        configHome: GLib.getenv("XDG_CONFIG_HOME"),
        root: paths.root,
      },
      createServiceSystem(files),
    );
    const poller = new DaemonPoller(client, glibTimers, {
      launcher,
      now: () => GLib.get_monotonic_time() / 1000,
    });
    poller.onUpdate = (result) => this._show(result);
    poller.onError = (error) => console.error(`Allot: updating the panel failed: ${error}`);
    poller.setPopupOpen(this._popup?.isOpen ?? false);
    this._poller = poller;
    connection.onOwnerChanged = () => poller.pollNow();
    connection.onJobsChanged = () => poller.pollNow();
    try {
      await poller.start();
    } catch (error) {
      console.error(`Allot: polling stopped: ${error}`);
    }
  }

  /**
   * Learns when the shell's startup animation is over, which the first-run popup waits for. The
   * shell offers no public flag, and the signal never comes for an extension turned on after
   * login, so a grace period ends the wait.
   */
  _watchStartup() {
    this._startupComplete = false;
    this._whenStarted = null;
    const done = () => {
      this._startupComplete = true;
      const whenStarted = this._whenStarted;
      this._whenStarted = null;
      whenStarted?.();
    };
    this._startupSignalId = Main.layoutManager.connect("startup-complete", () => {
      this._clearStartupTimer();
      done();
    });
    this._startupTimerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, STARTUP_GRACE_MS, () => {
      this._startupTimerId = 0;
      done();
      return GLib.SOURCE_REMOVE;
    });
  }

  _clearStartupTimer() {
    if (this._startupTimerId) {
      GLib.source_remove(this._startupTimerId);
      this._startupTimerId = 0;
    }
  }

  _clearStartupWatch() {
    this._clearStartupTimer();
    if (this._startupSignalId) {
      Main.layoutManager.disconnect(this._startupSignalId);
      this._startupSignalId = 0;
    }
    this._whenStarted = null;
  }

  /** @param {PollResult} result */
  _show(result) {
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
    if (!this._startupComplete) {
      this._whenStarted = () => this._openForFirstRun();
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

  /**
   * Raises the open Settings window, since `openPreferences()` refuses to while one is open and
   * does not raise it; opens one otherwise.
   */
  _openSettings() {
    this._popup?.close();
    const prefsWindows = global
      .get_window_actors()
      .map((actor) => actor.meta_window)
      .filter((window) => isPrefsService(window));
    const ours = prefsWindows.find((window) => window.get_title() === this.metadata.name);
    if (ours !== undefined) {
      Main.activateWindow(ours);
      return;
    }
    if (prefsWindows.length > 0) {
      // The shell keeps one preferences window for all extensions, so another one may be open.
      Main.notify(this.metadata.name, OTHER_PREFS_TEXT);
      return;
    }
    // GNOME 48 and 49 return a promise that rejects when the shell refuses to open the window;
    // GNOME 50 and later return nothing and report a refusal as an unhandled rejection.
    const opened = /** @type {unknown} */ (this.openPreferences());
    if (opened instanceof Promise) {
      opened.catch((error) => console.warn(`Allot: cannot open Settings: ${error}`));
    }
  }

  /**
   * Follows `tray.json` while the Settings window, in its own process, changes the panel's
   * settings: its atomic writes reach the monitor at once. Each burst of events is read
   * once.
   *
   * @param {import("./lib/config.js").DataPaths} paths
   */
  _watchSettings(paths) {
    this._settingsReadId = 0;
    this._settingsMonitor = Gio.File.new_for_path(paths.settings).monitor_file(
      Gio.FileMonitorFlags.WATCH_MOVES,
      null,
    );
    this._settingsMonitor.connect("changed", () => {
      if (this._settingsReadId) {
        GLib.source_remove(this._settingsReadId);
      }
      this._settingsReadId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTINGS_READ_DELAY_MS, () => {
        this._settingsReadId = 0;
        this._readSettings(paths);
        return GLib.SOURCE_REMOVE;
      });
    });
  }

  /** @param {import("./lib/config.js").DataPaths} paths */
  async _readSettings(paths) {
    const files = this._files;
    const text = await readTextFile(paths.settings, files);
    if (this._files !== files || this._model === null || text === null) {
      return;
    }
    const settings = decodeSettings(text);
    if (JSON.stringify(settings) !== JSON.stringify(this._model.settings)) {
      this._showChange(this._model.loadSettings(settings));
    }
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
          this._stopDialog = confirmStop(question, () => this._stopService());
          this._stopDialog.connect("destroy", () => {
            this._stopDialog = null;
          });
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
    // A disable cancels the checks, which then read as a missing allotd.
    if (problem !== null && this._poller === poller) {
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
      // Disabling the extension cancels the request, which is no failure.
      if (!(error instanceof CancelledError)) {
        console.warn(`Allot: stop not finished: ${error}`);
      }
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
    this._clearShowIdle();
    this._showIdleId = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
      this._showIdleId = 0;
      this._popup?.show(target);
      return GLib.SOURCE_REMOVE;
    });
  }

  _clearShowIdle() {
    if (this._showIdleId !== 0) {
      GLib.source_remove(this._showIdleId);
      this._showIdleId = 0;
    }
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

/**
 * Whether a window is the preferences service's. On Wayland the app id arrives as the window
 * class; the GTK application id may stay unset.
 *
 * @param {import("gi://Meta").default.Window} window
 * @returns {boolean}
 */
function isPrefsService(window) {
  return (
    window.get_wm_class() === PREFS_APP_ID || window.get_gtk_application_id() === PREFS_APP_ID
  );
}
