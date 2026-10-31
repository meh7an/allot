import GLib from "gi://GLib";

import { Extension } from "resource:///org/gnome/shell/extensions/extension.js";
import * as Main from "resource:///org/gnome/shell/ui/main.js";

import { SoupTransport, glibTimers, readTextFile } from "./daemon-io.js";
import { AllotIndicator } from "./indicator.js";
import { DaemonClient } from "./lib/client.js";
import { DEFAULT_PORT, dataPaths, portFromToml } from "./lib/config.js";
import { DaemonPoller } from "./lib/poller.js";
import { PopupModel } from "./lib/popup-model.js";
import { AllotPopup } from "./popup.js";

/** @import { PollResult } from "./lib/client.js" */

export default class AllotExtension extends Extension {
  enable() {
    const paths = dataPaths(GLib.get_home_dir());
    const iconDirectory = this.dir.get_child("icons");
    this._model = new PopupModel();
    this._transport = new SoupTransport();
    this._indicator = new AllotIndicator(iconDirectory, () => this._popup?.toggle());
    this._popup = new AllotPopup({
      model: this._model,
      indicator: this._indicator,
      iconDirectory,
      logsPath: paths.logs,
      onOpenChanged: (open) => this._poller?.setPopupOpen(open),
    });
    this._indicator.showStatus(this._model.indicator);
    Main.panel.addToStatusArea(this.uuid, this._indicator);
    this._lastStatus = null;
    this._startPolling(paths, this._transport);
  }

  disable() {
    this._poller?.stop();
    this._transport?.cancel();
    this._popup?.destroy();
    this._indicator?.destroy();
    this._poller = null;
    this._transport = null;
    this._popup = null;
    this._indicator = null;
    this._model = null;
  }

  /**
   * Reads the port once, as the other apps do at launch, then polls until `disable`.
   *
   * @param {import("./lib/config.js").DataPaths} paths
   * @param {SoupTransport} transport
   */
  async _startPolling(paths, transport) {
    const toml = await readTextFile(paths.config);
    // A disable, or a disable and enable, while the config was being read.
    if (this._transport !== transport) {
      return;
    }
    const client = new DaemonClient({
      port: toml === null ? DEFAULT_PORT : portFromToml(toml),
      send: transport.send,
      readToken: () => readTextFile(paths.token),
    });
    const poller = new DaemonPoller(client, glibTimers);
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
    const change = this._model.apply(result, this._poller?.lastKnown ?? null);
    if (change.indicator) {
      this._indicator.showStatus(this._model.indicator);
    }
    if (change.snapshot && this._popup.isOpen) {
      this._popup.render();
    }
  }
}
