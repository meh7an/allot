import Clutter from "gi://Clutter";
import Gio from "gi://Gio";
import GLib from "gi://GLib";
import Pango from "gi://Pango";
import Shell from "gi://Shell";
import St from "gi://St";

import * as BoxPointer from "resource:///org/gnome/shell/ui/boxpointer.js";
import * as Main from "resource:///org/gnome/shell/ui/main.js";
import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";

import { anchoredPosition, clampInto, maxHeight } from "./lib/placement.js";

/** @import { PopupModel } from "./lib/popup-model.js" */
/** @import { HeaderGlyph, JobGroup, JobRow } from "./lib/snapshot.js" */

const GROUP_ICONS = {
  benchmark: "alarm-symbolic",
  running: "media-playback-start-symbolic",
  waiting: "appointment-soon-symbolic",
};
const NOTHING_TEXT = "No jobs running or waiting.";
const OPAQUE = 255;
const DIMMED = 128;
// The popup's padding and the header's bottom gap, in CSS pixels, which the list's height cap
// leaves room for.
const CHROME_HEIGHT = 28;

/**
 * The Allot popup: a header with the summary and the Stay open, Pin and More options buttons,
 * and the job list. It is shell chrome rather than a panel menu, because a panel menu always
 * holds the pointer and closes on a click elsewhere, which rules out Stay open and Pin.
 */
export class AllotPopup {
  #model;
  #indicator;
  #markIcon;
  #logsPath;
  #onOpenChanged;

  #actor;
  #header;
  #glyph;
  #summary;
  #pinButton;
  #stayOpenButton;
  #stayOpenIcon;
  #moreButton;
  #scroll;
  #list;
  #menuManager;
  #moreMenu;

  /** @type {Clutter.Grab | null} */
  #modal = null;
  /** @type {{ grab: Clutter.Grab, dx: number, dy: number } | null} */
  #drag = null;
  #monitorsChangedId;

  /**
   * @param {object} options
   * @param {PopupModel} options.model
   * @param {InstanceType<typeof import("./indicator.js").AllotIndicator>} options.indicator
   * @param {Gio.File} options.iconDirectory
   * @param {string} options.logsPath
   * @param {(open: boolean) => void} options.onOpenChanged
   */
  constructor(options) {
    this.#model = options.model;
    this.#indicator = options.indicator;
    this.#markIcon = new Gio.FileIcon({
      file: options.iconDirectory.get_child("allot-mark-symbolic.svg"),
    });
    this.#logsPath = options.logsPath;
    this.#onOpenChanged = options.onOpenChanged;

    this.#actor = new St.BoxLayout({
      style_class: "popup-menu-content allot-popup",
      orientation: Clutter.Orientation.VERTICAL,
      reactive: true,
      visible: false,
    });
    this.#actor.connect("captured-event", (_actor, event) => this.#onCapturedEvent(event));

    this.#glyph = new St.Icon({ style_class: "allot-header-glyph" });
    this.#summary = new St.Label({
      style_class: "allot-summary",
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });
    this.#summary.clutter_text.ellipsize = Pango.EllipsizeMode.END;
    this.#pinButton = headerButton("Pin", new St.Icon({ icon_name: "view-pin-symbolic" }));
    this.#pinButton.connect("clicked", () => this.#togglePinned());
    this.#stayOpenIcon = new St.Icon();
    this.#stayOpenButton = headerButton("Stay open", this.#stayOpenIcon);
    this.#stayOpenButton.connect("clicked", () => this.#toggleStayOpen());
    this.#moreButton = headerButton(
      "More options",
      new St.Icon({ icon_name: "view-more-horizontal-symbolic" }),
    );

    // Empty header space is the drag handle while pinned; the buttons keep their own presses.
    this.#header = new St.BoxLayout({ style_class: "allot-header", reactive: true });
    this.#header.connect("button-press-event", (_actor, event) => this.#startDrag(event));
    this.#header.connect("motion-event", (_actor, event) => this.#dragTo(event));
    this.#header.connect("button-release-event", () => this.#endDrag());
    for (const child of [
      this.#glyph,
      this.#summary,
      this.#pinButton,
      this.#stayOpenButton,
      this.#moreButton,
    ]) {
      this.#header.add_child(child);
    }

    this.#list = new St.BoxLayout({
      style_class: "allot-list",
      orientation: Clutter.Orientation.VERTICAL,
    });
    this.#scroll = new St.ScrollView({
      style_class: "allot-scroll",
      hscrollbar_policy: St.PolicyType.NEVER,
      vscrollbar_policy: St.PolicyType.AUTOMATIC,
      overlay_scrollbars: true,
      child: this.#list,
    });
    this.#actor.add_child(this.#header);
    this.#actor.add_child(this.#scroll);
    Main.layoutManager.addTopChrome(this.#actor);

    this.#menuManager = new PopupMenu.PopupMenuManager(this.#actor);
    this.#moreMenu = new PopupMenu.PopupMenu(this.#moreButton, 0.5, St.Side.TOP);
    this.#moreMenu.addAction("Open Logs", () => this.#openLogs());
    Main.uiGroup.add_child(this.#moreMenu.actor);
    this.#moreMenu.actor.hide();
    this.#menuManager.addMenu(this.#moreMenu);
    this.#moreButton.connect("clicked", () => this.#moreMenu.toggle());

    this.#monitorsChangedId = Main.layoutManager.connect("monitors-changed", () =>
      this.#keepOnScreen(),
    );
    this.#showMode();
  }

  /** @returns {boolean} */
  get isOpen() {
    return this.#actor.visible;
  }

  toggle() {
    if (this.isOpen) {
      this.close();
    } else {
      this.open();
    }
  }

  /** Opens under the panel button, always in light-dismiss mode. */
  open() {
    this.#model.resetForOpen();
    this.#actor.show();
    this.render();
    this.#applyMode();
    this.#indicator.setOpen(true);
    this.#onOpenChanged(true);
  }

  close() {
    if (!this.isOpen) {
      return;
    }
    this.#moreMenu.close(BoxPointer.PopupAnimation.NONE);
    this.#endDrag();
    this.#releaseModal();
    this.#actor.hide();
    this.#indicator.setOpen(false);
    this.#onOpenChanged(false);
  }

  /** Shows the model's snapshot; the panel button is updated separately. */
  render() {
    const { header, groups, idle, showNothing, isLastKnown } = this.#model.snapshot;
    this.#summary.text = header.summary;
    this.#showGlyph(header.glyph);

    this.#list.destroy_all_children();
    for (const group of groups) {
      this.#list.add_child(groupHeading(group));
      for (const job of group.jobs) {
        this.#list.add_child(jobRow(job));
      }
    }
    if (idle !== null) {
      const line = [idle.label, idle.ids, idle.usage].filter((part) => part !== "").join("  ");
      this.#list.add_child(label("allot-idle-line", line));
    }
    if (showNothing) {
      this.#list.add_child(label("allot-nothing", NOTHING_TEXT));
    }
    this.#list.opacity = isLastKnown ? DIMMED : OPAQUE;
    this.#scroll.visible = this.#list.get_n_children() > 0;

    if (this.isOpen && !this.#model.isPinned) {
      this.#placeAnchored();
    }
  }

  destroy() {
    this.close();
    Main.layoutManager.disconnect(this.#monitorsChangedId);
    this.#moreMenu.destroy();
    Main.layoutManager.removeChrome(this.#actor);
    this.#actor.destroy();
  }

  /** @param {HeaderGlyph} glyph */
  #showGlyph(glyph) {
    if (glyph === "error") {
      this.#glyph.icon_name = "dialog-error-symbolic";
      this.#glyph.add_style_class_name("allot-header-glyph-error");
      this.#glyph.opacity = OPAQUE;
      return;
    }
    this.#glyph.gicon = this.#markIcon;
    this.#glyph.remove_style_class_name("allot-header-glyph-error");
    this.#glyph.opacity = glyph === "idle" ? DIMMED : OPAQUE;
  }

  #toggleStayOpen() {
    const wasPinned = this.#model.isPinned;
    this.#model.toggleStayOpen();
    this.#applyMode();
    if (wasPinned) {
      this.#placeAnchored();
    }
  }

  #togglePinned() {
    this.#model.togglePinned();
    this.#applyMode();
    if (!this.#model.isPinned) {
      this.#placeAnchored();
    }
  }

  #applyMode() {
    if (this.#model.isStayOpen) {
      this.#releaseModal();
    } else {
      this.#takeModal();
    }
    this.#showMode();
  }

  #showMode() {
    const { isStayOpen, isPinned } = this.#model;
    this.#stayOpenButton.checked = isStayOpen;
    this.#stayOpenIcon.icon_name = isStayOpen
      ? "changes-prevent-symbolic"
      : "changes-allow-symbolic";
    this.#pinButton.visible = isStayOpen;
    this.#pinButton.can_focus = isStayOpen;
    this.#pinButton.checked = isPinned;
    if (isPinned) {
      this.#header.add_style_class_name("allot-header-pinned");
    } else {
      this.#header.remove_style_class_name("allot-header-pinned");
    }
  }

  // Light dismiss holds the pointer and keyboard like a panel menu does, so a click on a window,
  // the panel or the panel button lands here first and closes the popup.
  #takeModal() {
    if (this.#modal === null) {
      this.#modal = Main.pushModal(this.#actor, { actionMode: Shell.ActionMode.POPUP });
    }
  }

  #releaseModal() {
    if (this.#modal !== null) {
      Main.popModal(this.#modal);
      this.#modal = null;
    }
  }

  /** @param {Clutter.Event} event */
  #onCapturedEvent(event) {
    if (this.#modal === null) {
      return Clutter.EVENT_PROPAGATE;
    }
    const type = event.type();
    if (type === Clutter.EventType.KEY_PRESS && event.get_key_symbol() === Clutter.KEY_Escape) {
      this.close();
      return Clutter.EVENT_STOP;
    }
    const isPress =
      type === Clutter.EventType.BUTTON_PRESS || type === Clutter.EventType.TOUCH_BEGIN;
    if (isPress && !this.#actor.contains(global.stage.get_event_actor(event))) {
      this.close();
      return Clutter.EVENT_STOP;
    }
    return Clutter.EVENT_PROPAGATE;
  }

  #placeAnchored() {
    const monitor = Main.layoutManager.findIndexForActor(this.#indicator);
    const workArea = Main.layoutManager.getWorkAreaForMonitor(monitor);
    this.#capHeight(workArea);
    const [x, y] = this.#indicator.get_transformed_position();
    const [width, height] = this.#indicator.get_transformed_size();
    const position = anchoredPosition({ x, y, width, height }, this.#size(), workArea);
    this.#actor.set_position(position.x, position.y);
  }

  #keepOnScreen() {
    if (!this.isOpen) {
      return;
    }
    if (!this.#model.isPinned) {
      this.#placeAnchored();
      return;
    }
    const workArea = this.#workAreaAt(this.#actor.x, this.#actor.y);
    this.#capHeight(workArea);
    const position = clampInto({ x: this.#actor.x, y: this.#actor.y }, this.#size(), workArea);
    this.#actor.set_position(position.x, position.y);
  }

  /** @param {{ height: number }} workArea */
  #capHeight(workArea) {
    const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
    const [, headerHeight] = this.#header.get_preferred_height(-1);
    const listHeight = (maxHeight(workArea) - headerHeight) / scale - CHROME_HEIGHT;
    this.#scroll.style = `max-height: ${Math.max(0, Math.floor(listHeight))}px;`;
  }

  /** @returns {{ width: number, height: number }} */
  #size() {
    const [, width] = this.#actor.get_preferred_width(-1);
    const [, height] = this.#actor.get_preferred_height(width);
    return { width, height };
  }

  /**
   * @param {number} x
   * @param {number} y
   */
  #workAreaAt(x, y) {
    const { monitors, primaryIndex } = Main.layoutManager;
    const index = monitors.findIndex(
      (monitor) =>
        x >= monitor.x &&
        x < monitor.x + monitor.width &&
        y >= monitor.y &&
        y < monitor.y + monitor.height,
    );
    return Main.layoutManager.getWorkAreaForMonitor(index < 0 ? primaryIndex : index);
  }

  /** @param {Clutter.Event} event */
  #startDrag(event) {
    if (!this.#model.isPinned || event.get_button() !== Clutter.BUTTON_PRIMARY) {
      return Clutter.EVENT_PROPAGATE;
    }
    const [x, y] = event.get_coords();
    this.#drag = {
      grab: global.stage.grab(this.#header),
      dx: x - this.#actor.x,
      dy: y - this.#actor.y,
    };
    return Clutter.EVENT_STOP;
  }

  /** @param {Clutter.Event} event */
  #dragTo(event) {
    if (this.#drag === null) {
      return Clutter.EVENT_PROPAGATE;
    }
    const [x, y] = event.get_coords();
    const wanted = { x: x - this.#drag.dx, y: y - this.#drag.dy };
    const position = clampInto(wanted, this.#size(), this.#workAreaAt(x, y));
    this.#actor.set_position(position.x, position.y);
    return Clutter.EVENT_STOP;
  }

  #endDrag() {
    if (this.#drag === null) {
      return Clutter.EVENT_PROPAGATE;
    }
    this.#drag.grab.dismiss();
    this.#drag = null;
    return Clutter.EVENT_STOP;
  }

  #openLogs() {
    const uri = GLib.filename_to_uri(this.#logsPath, null);
    try {
      Gio.AppInfo.launch_default_for_uri(uri, global.create_app_launch_context(0, -1));
    } catch (error) {
      console.warn(`Allot: cannot open ${uri}: ${error.message}`);
    }
    if (!this.#model.isStayOpen) {
      this.close();
    }
  }
}

/**
 * @param {string} name The tooltip-free accessible name.
 * @param {St.Icon} icon
 * @returns {St.Button}
 */
function headerButton(name, icon) {
  return new St.Button({
    style_class: "icon-button allot-header-button",
    accessible_name: name,
    can_focus: true,
    child: icon,
  });
}

/**
 * @param {string} styleClass
 * @param {string} text
 * @returns {St.Label}
 */
function label(styleClass, text) {
  const result = new St.Label({ style_class: styleClass, text, x_expand: true });
  result.clutter_text.ellipsize = Pango.EllipsizeMode.END;
  return result;
}

/**
 * @param {JobGroup} group
 * @returns {St.BoxLayout}
 */
function groupHeading(group) {
  const row = new St.BoxLayout({ style_class: "allot-group-heading" });
  row.add_child(new St.Icon({ icon_name: GROUP_ICONS[group.kind] }));
  row.add_child(label("allot-group-label", group.heading));
  return row;
}

/**
 * @param {JobRow} job
 * @returns {St.BoxLayout}
 */
function jobRow(job) {
  const row = new St.BoxLayout({
    style_class: job.isQueued ? "allot-job allot-job-queued" : "allot-job",
    orientation: Clutter.Orientation.VERTICAL,
  });
  const { shortId, project, usage } = job.session;
  const strip = [shortId, project, usage].filter((part) => part !== "").join(" · ");
  row.add_child(label("allot-job-strip", strip));
  row.add_child(label("allot-job-command", job.command));
  return row;
}
