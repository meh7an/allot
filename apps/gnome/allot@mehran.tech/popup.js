import Atk from "gi://Atk";
import Clutter from "gi://Clutter";
import Gio from "gi://Gio";
import GLib from "gi://GLib";
import Pango from "gi://Pango";
import Shell from "gi://Shell";
import St from "gi://St";

import { ensureActorVisibleInScrollView } from "resource:///org/gnome/shell/misc/animationUtils.js";
import * as BoxPointer from "resource:///org/gnome/shell/ui/boxpointer.js";
import * as Main from "resource:///org/gnome/shell/ui/main.js";
import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";

import { AppIcons } from "./app-icons.js";
import {
  GroupHeading,
  JobCardView,
  RecentView,
  ServiceBannerView,
  WelcomeView,
  collapse,
  dimmedOpacity,
  fadeIn,
  label,
  setClass,
} from "./cards.js";
import { Announcer } from "./lib/accessible-text.js";
import { APP_NAME } from "./lib/indicator-status.js";
import { anchoredPosition, clampInto, maxHeight } from "./lib/placement.js";
import { MoreOptionsMenu } from "./menus.js";
import { ResourcesCard } from "./resources-card.js";

/** @import { PopupModel } from "./lib/popup-model.js" */
/** @import { HeaderGlyph } from "./lib/snapshot.js" */

/**
 * A row of the job list: a key that stays the same while the row is shown, how to make its
 * view, and how to show the current snapshot in it.
 *
 * @typedef {object} ListItem
 * @property {string} key
 * @property {() => { actor: Clutter.Actor, update: Function }} make
 * @property {(view: any) => void} update
 */

const NOTHING_TEXT = "No jobs running or waiting.";
const OPAQUE = 255;
const DIMMED = 128;
const LAST_KNOWN_OPACITY = 140;
// The popup's padding and the header's bottom gap, in CSS pixels, which the list's height cap
// leaves room for.
const CHROME_HEIGHT = 28;
// The popup's spacing above a banner or the Resources card, in CSS pixels.
const BLOCK_GAP = 8;
// Ctrl+Alt+Tab lists the open popup under this icon, the one way a keyboard reaches it while
// Stay open has given up the modal grab.
const SWITCHER_ICON = "view-pin-symbolic";
const TAB_DIRECTIONS = {
  [Clutter.KEY_Tab]: St.DirectionType.TAB_FORWARD,
  [Clutter.KEY_ISO_Left_Tab]: St.DirectionType.TAB_BACKWARD,
};

/**
 * The Allot popup: a header with the summary and the Stay open, Pin and More options buttons,
 * the service banner, the job list and the docked Resources card. It is shell chrome rather than
 * a panel menu, because a panel menu always holds the pointer and closes on a click elsewhere,
 * which rules out Stay open and Pin.
 */
export class AllotPopup {
  #model;
  #indicator;
  #markIcon;
  #logsPath;
  #onOpenChanged;
  #onChanged;
  #announcer = new Announcer();
  #appIcons = new AppIcons();
  /** @type {Map<string, { actor: Clutter.Actor, update: Function }>} */
  #views = new Map();
  /** @type {import("./cards.js").CardActions} */
  #cardActions;

  #actor;
  #header;
  #glyph;
  #summary;
  #pinButton;
  #stayOpenButton;
  #stayOpenIcon;
  #moreButton;
  #banner;
  #scroll;
  #list;
  #resources;
  #menuManager;
  #more;
  /** @type {string | null} */
  #listCap = null;
  #scrollWait = 0;

  /** @type {Clutter.Grab | null} */
  #modal = null;
  /** @type {{ grab: Clutter.Grab, dx: number, dy: number } | null} */
  #drag = null;
  #monitorsChangedId;
  #highContrastId;
  #contrastSheet;
  #contrastLoaded = false;

  /**
   * @param {object} options
   * @param {PopupModel} options.model
   * @param {InstanceType<typeof import("./indicator.js").AllotIndicator>} options.indicator
   * @param {Gio.File} options.iconDirectory
   * @param {Gio.File} options.contrastSheet The stylesheet to add while high contrast is on.
   * @param {string} options.logsPath
   * @param {(open: boolean) => void} options.onOpenChanged
   * @param {() => void} options.onChanged A press or Esc changed the model's stops.
   * @param {() => void} options.onStartService The banner's Start Service was pressed.
   */
  constructor(options) {
    this.#model = options.model;
    this.#indicator = options.indicator;
    this.#markIcon = new Gio.FileIcon({
      file: options.iconDirectory.get_child("allot-mark-symbolic.svg"),
    });
    this.#contrastSheet = options.contrastSheet;
    this.#logsPath = options.logsPath;
    this.#onOpenChanged = options.onOpenChanged;
    this.#onChanged = options.onChanged;
    this.#cardActions = {
      kill: (lease) => this.#pressKill(lease),
      remove: (lease) => this.#pressRemove(lease),
      cancel: (lease) => {
        if (this.#model.cancelConfirm(lease)) {
          this.#onChanged();
        }
      },
      focused: (actor) => ensureActorVisibleInScrollView(this.#scroll, actor),
      appIcon: this.#appIcons.icon,
    };

    this.#actor = new St.BoxLayout({
      style_class: "popup-menu-content allot-popup",
      orientation: Clutter.Orientation.VERTICAL,
      reactive: true,
      visible: false,
      accessible_name: APP_NAME,
      accessible_role: Atk.Role.DIALOG,
    });
    this.#actor.connect("captured-event", (_actor, event) => this.#onCapturedEvent(event));

    this.#glyph = new St.Icon({ style_class: "allot-header-glyph" });
    this.#summary = new St.Label({
      style_class: "allot-summary",
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });
    this.#summary.clutter_text.ellipsize = Pango.EllipsizeMode.END;
    this.#pinButton = headerButton(
      "Pin",
      new St.Icon({ icon_name: "view-pin-symbolic" }),
      Atk.Role.TOGGLE_BUTTON,
    );
    this.#pinButton.connect("clicked", () => this.#togglePinned());
    this.#stayOpenIcon = new St.Icon();
    this.#stayOpenButton = headerButton("Stay open", this.#stayOpenIcon, Atk.Role.TOGGLE_BUTTON);
    this.#stayOpenButton.connect("clicked", () => this.#toggleStayOpen());
    this.#moreButton = headerButton(
      "More options",
      new St.Icon({ icon_name: "view-more-horizontal-symbolic" }),
      Atk.Role.PUSH_BUTTON,
    );
    this.#moreButton.add_accessible_state(Atk.StateType.EXPANDABLE);

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

    this.#banner = new ServiceBannerView(options.onStartService);
    this.#resources = new ResourcesCard({
      onToggle: () => {
        this.#model.toggleResources();
        this.#onChanged();
      },
      onHeightChanged: () => this.#followHeight(),
    });

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
    this.#actor.add_child(this.#banner.actor);
    this.#actor.add_child(this.#scroll);
    this.#actor.add_child(this.#resources.actor);
    Main.layoutManager.addTopChrome(this.#actor);

    this.#menuManager = new PopupMenu.PopupMenuManager(this.#actor);
    this.#more = new MoreOptionsMenu({
      source: this.#moreButton,
      model: this.#model,
      onOpenLogs: () => this.openLogs(),
      onChanged: () => this.#onChanged(),
    });
    Main.uiGroup.add_child(this.#more.menu.actor);
    this.#more.menu.actor.hide();
    this.#menuManager.addMenu(this.#more.menu);
    this.#moreButton.connect("clicked", () => this.#more.menu.toggle());
    this.#more.menu.connect("open-state-changed", (_menu, open) => {
      if (open) {
        this.#moreButton.add_accessible_state(Atk.StateType.EXPANDED);
      } else {
        this.#moreButton.remove_accessible_state(Atk.StateType.EXPANDED);
      }
    });

    this.#monitorsChangedId = Main.layoutManager.connect("monitors-changed", () =>
      this.#keepOnScreen(),
    );
    this.#highContrastId = St.Settings.get().connect("notify::high-contrast", () =>
      this.#showContrast(),
    );
    this.#showContrast();
    this.#showMode();
  }

  /** @returns {boolean} */
  get isOpen() {
    return this.#actor.visible;
  }

  /** @param {boolean} [byKeyboard] */
  toggle(byKeyboard = false) {
    if (this.isOpen) {
      this.close();
    } else {
      this.open(byKeyboard);
    }
  }

  /**
   * Opens under the panel button, always in light-dismiss mode. A keyboard user lands on the first
   * control; a pointer user does not get a focus ring they did not ask for, and Tab still starts
   * there.
   *
   * @param {boolean} [byKeyboard]
   */
  open(byKeyboard = false) {
    this.#model.resetForOpen();
    this.#announcer.reset();
    this.#actor.show();
    this.render(false);
    this.#applyMode();
    Main.ctrlAltTabManager.addGroup(this.#actor, APP_NAME, SWITCHER_ICON);
    if (byKeyboard) {
      this.#stayOpenButton.grab_key_focus();
    }
    this.#indicator.setOpen(true);
    this.#onOpenChanged(true);
  }

  /**
   * Opens if needed and scrolls to a row, for a notification's Show.
   *
   * @param {string | null} target A row's key (`job:<lease>` or `recent`), or `null` for the top.
   */
  show(target) {
    if (!this.isOpen) {
      this.open();
    }
    // The row has no position, and the scroll range no size, until a frame has laid them out.
    this.#clearScrollWait();
    this.#actor.queue_redraw();
    this.#scrollWait = global.stage.connect("after-paint", () => {
      this.#clearScrollWait();
      const row = target === null ? undefined : this.#views.get(target);
      this.#scroll.vadjustment.value = row?.actor.y ?? 0;
    });
  }

  /** Shows the settings' current state in the menu, for a change from outside it. */
  syncMenu() {
    this.#more.sync();
  }

  close() {
    if (!this.isOpen) {
      return;
    }
    this.#clearScrollWait();
    this.#resources.hideHover();
    this.#more.menu.close(BoxPointer.PopupAnimation.NONE);
    this.#endDrag();
    this.#releaseModal();
    Main.ctrlAltTabManager.removeGroup(this.#actor);
    this.#actor.hide();
    this.#cancelConfirms();
    this.#indicator.setOpen(false);
    this.#onOpenChanged(false);
  }

  /**
   * Shows the model's snapshot; the panel button is updated separately. Rows are updated in
   * place by key, so a ticking time or a countdown keeps the Kill? button and its focus.
   *
   * @param {boolean} [animate] Fade rows in and out and ease the bars; off when the popup opens.
   */
  render(animate = true) {
    const { header, groups, idle, recent, showNothing, isLastKnown, welcome } =
      this.#model.snapshot;
    this.#banner.update(this.#model.banner, animate);
    this.#resources.update(this.#model.resources, animate);
    this.#more.sync();
    this.#summary.text = header.summary;
    this.#showGlyph(header.glyph);

    /** @type {ListItem[]} */
    const items = [];
    for (const group of groups) {
      items.push({
        key: `group:${group.kind}`,
        make: () => new GroupHeading(group),
        update: (view) => view.update(group),
      });
      for (const job of group.jobs) {
        items.push({
          key: `job:${job.id}`,
          make: () => new JobCardView(this.#cardActions),
          update: (view) => view.update(job, animate),
        });
      }
    }
    if (idle !== null) {
      const line = [idle.label, idle.ids, idle.usage].filter((part) => part !== "").join("  ");
      items.push(textItem("idle", "allot-idle-line", line));
    }
    if (welcome !== null) {
      items.push({
        key: "welcome",
        make: () =>
          new WelcomeView(() => {
            this.#model.finishFirstRun();
            this.#onChanged();
          }),
        update: (view) => view.update(welcome),
      });
    }
    if (recent.length > 0) {
      items.push({
        key: "recent",
        make: () => new RecentView(),
        update: (view) => view.update(recent),
      });
    } else if (showNothing && welcome === null) {
      items.push(textItem("nothing", "allot-nothing", NOTHING_TEXT));
    }
    this.#showItems(items, animate);
    this.#list.opacity = isLastKnown ? dimmedOpacity(LAST_KNOWN_OPACITY) : OPAQUE;
    this.#scroll.visible = items.length > 0;

    if (this.isOpen && !this.#model.isPinned) {
      this.#placeAnchored();
    }
    this.#moveFocusOffDeadControl();
    for (const text of this.#announcer.observe(this.#model.snapshot)) {
      this.#actor.get_accessible()?.emit("notification", text, Atk.Live.POLITE);
    }
  }

  destroy() {
    this.close();
    this.#appIcons.destroy();
    Main.layoutManager.disconnect(this.#monitorsChangedId);
    St.Settings.get().disconnect(this.#highContrastId);
    if (this.#contrastLoaded) {
      const theme = St.ThemeContext.get_for_stage(global.stage).get_theme();
      theme.unload_stylesheet(this.#contrastSheet);
    }
    this.#more.menu.destroy();
    this.#resources.destroy();
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

  #showContrast() {
    const on = St.Settings.get().high_contrast;
    if (on !== this.#contrastLoaded) {
      const theme = St.ThemeContext.get_for_stage(global.stage).get_theme();
      if (on) {
        theme.load_stylesheet(this.#contrastSheet);
      } else {
        theme.unload_stylesheet(this.#contrastSheet);
      }
      this.#contrastLoaded = on;
    }
    setClass(this.#actor, "allot-high-contrast", on);
  }

  #toggleStayOpen() {
    const wasPinned = this.#model.isPinned;
    const hadFocus = this.#stayOpenButton.has_key_focus();
    this.#model.toggleStayOpen();
    this.#applyMode();
    // Releasing the modal grab hands focus back to where it came from, out of the popup.
    if (hadFocus) {
      this.#stayOpenButton.grab_key_focus();
    }
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
    const type = event.type();
    // Esc first takes back a Kill? confirm, then closes a light-dismiss popup.
    if (type === Clutter.EventType.KEY_PRESS && event.get_key_symbol() === Clutter.KEY_Escape) {
      if (this.#model.isConfirming) {
        this.#cancelConfirms();
        return Clutter.EVENT_STOP;
      }
      if (this.#modal !== null) {
        this.close();
        return Clutter.EVENT_STOP;
      }
      return Clutter.EVENT_PROPAGATE;
    }
    if (this.#modal === null) {
      return Clutter.EVENT_PROPAGATE;
    }
    // The grab keeps key presses from the stage, where St's focus manager handles Tab.
    if (type === Clutter.EventType.KEY_PRESS && event.get_key_symbol() in TAB_DIRECTIONS) {
      const focus = global.stage.key_focus;
      this.#actor.navigate_focus(
        focus !== null && this.#inPopup(focus) ? focus : null,
        TAB_DIRECTIONS[event.get_key_symbol()],
        true,
      );
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

  /**
   * The Resources card folded a step: the list makes room for it frame by frame, and a pinned
   * popup that grew stays on screen.
   */
  #followHeight() {
    if (this.isOpen) {
      this.#keepOnScreen();
    }
  }

  /** @param {{ height: number }} workArea */
  #capHeight(workArea) {
    const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
    const [, width] = this.#actor.get_preferred_width(-1);
    let taken = this.#header.get_preferred_height(-1)[1];
    for (const block of [this.#banner.actor, this.#resources.actor]) {
      if (block.visible) {
        taken += block.get_preferred_height(width)[1] + BLOCK_GAP * scale;
      }
    }
    const listHeight = (maxHeight(workArea) - taken) / scale - CHROME_HEIGHT;
    const cap = `max-height: ${Math.max(0, Math.floor(listHeight))}px;`;
    // Setting a style restyles the list, which a frame of folding should not pay for when nothing
    // changed.
    if (cap !== this.#listCap) {
      this.#listCap = cap;
      this.#scroll.style = cap;
    }
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

  /**
   * Puts the rows in order, reusing the view of every key shown before. A row whose key is
   * gone collapses where it was, so the rows around it do not jump.
   *
   * @param {ListItem[]} items
   * @param {boolean} animate
   */
  #showItems(items, animate) {
    const shown = new Set();
    /** @type {Clutter.Actor | null} */
    let previous = null;
    for (const item of items) {
      let view = this.#views.get(item.key);
      if (view === undefined) {
        view = item.make();
        this.#views.set(item.key, view);
        this.#list.add_child(view.actor);
        if (animate) {
          fadeIn(view.actor);
        }
      }
      if (previous === null) {
        this.#list.set_child_at_index(view.actor, 0);
      } else {
        this.#list.set_child_above_sibling(view.actor, previous);
      }
      item.update(view);
      shown.add(item.key);
      previous = view.actor;
    }
    for (const [key, view] of [...this.#views]) {
      if (shown.has(key)) {
        continue;
      }
      this.#views.delete(key);
      this.#moveFocusOutOf(view.actor);
      if (animate) {
        collapse(view.actor);
      } else {
        view.actor.destroy();
      }
    }
  }

  /**
   * A Kill just sent, or the Remove of a request that started, is no control any more. Without
   * this the keyboard would be stranded on it, and Tab would start over from the popup.
   */
  #moveFocusOffDeadControl() {
    const focus = global.stage.key_focus;
    if (focus instanceof St.Widget && !focus.can_focus && this.#inPopup(focus)) {
      this.#actor.navigate_focus(focus, St.DirectionType.TAB_FORWARD, true);
    }
  }

  /** @param {Clutter.Actor} leaving */
  #moveFocusOutOf(leaving) {
    const focus = global.stage.key_focus;
    if (focus instanceof St.Widget && leaving.contains(focus)) {
      focus.can_focus = false;
      this.#actor.navigate_focus(focus, St.DirectionType.TAB_FORWARD, true);
    }
  }

  /**
   * @param {Clutter.Actor} actor
   * @returns {boolean} A control of the popup, rather than the popup itself.
   */
  #inPopup(actor) {
    return actor !== this.#actor && this.#actor.contains(actor);
  }

  /** @param {number} lease */
  #pressKill(lease) {
    this.#model.pressKill(lease);
    this.#onChanged();
  }

  /** @param {number} lease */
  #pressRemove(lease) {
    this.#model.pressRemove(lease);
    this.#onChanged();
  }

  #cancelConfirms() {
    if (this.#model.cancelConfirms()) {
      this.#onChanged();
    }
  }

  #clearScrollWait() {
    if (this.#scrollWait !== 0) {
      global.stage.disconnect(this.#scrollWait);
      this.#scrollWait = 0;
    }
  }

  /** Opens the log folder, from a menu or a notification. */
  openLogs() {
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
 * @param {Atk.Role} role
 * @returns {St.Button}
 */
function headerButton(name, icon, role) {
  return new St.Button({
    style_class: "icon-button allot-header-button",
    accessible_name: name,
    accessible_role: role,
    can_focus: true,
    child: icon,
  });
}

/**
 * A plain line, such as the idle line.
 *
 * @param {string} key
 * @param {string} styleClass
 * @param {string} text
 * @returns {ListItem}
 */
function textItem(key, styleClass, text) {
  return {
    key,
    make: () => {
      const line = label(styleClass, text);
      line.x_expand = true;
      return {
        actor: line,
        update: () => {
          line.text = text;
        },
      };
    },
    update: (view) => view.update(),
  };
}
