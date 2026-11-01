import Clutter from "gi://Clutter";
import GLib from "gi://GLib";
import Pango from "gi://Pango";
import St from "gi://St";

import { cardDescription, detailText } from "./lib/accessible-text.js";
import { QUIET_TICKS } from "./lib/benchmark-status.js";
import { DO_NOT_DISTURB_TEXT, WELCOME_BODY, WELCOME_TITLE } from "./lib/snapshot.js";

/** @import { JobCard } from "./lib/job-card.js" */
/** @import { ServiceBanner } from "./lib/service-banner.js" */
/** @import { JobGroup, JobRow, RecentLine, WelcomeCard } from "./lib/snapshot.js" */

const GROUP_ICONS = {
  benchmark: "alarm-symbolic",
  running: "media-playback-start-symbolic",
  waiting: "appointment-soon-symbolic",
};
const FADE_MS = 240;
const BAR_MS = 900;
const STEP_RUN_MS = 700;
const ENDED_OPACITY = 128;
const DASH_PX = 6;

/**
 * Turns a card's action press into a model call: Kill and Kill? press Kill, Remove presses
 * Remove.
 *
 * @typedef {object} CardActions
 * @property {(lease: number) => void} kill
 * @property {(lease: number) => void} remove
 * @property {(lease: number) => void} cancel
 *   Focus left a Kill? confirm.
 * @property {(actor: Clutter.Actor) => void} focused
 *   The keyboard reached this card, which may be scrolled out of sight.
 * @property {(exe: string | null) => import("gi://Gio").Icon | null} appIcon
 */

/**
 * How opaque a dimmed part stays: dimming says nothing in high contrast, where the word beside it
 * does the work and the text must keep its full contrast.
 *
 * @param {number} dimmed
 * @returns {number}
 */
export function dimmedOpacity(dimmed) {
  return St.Settings.get().high_contrast ? 255 : dimmed;
}

/**
 * @param {string} styleClass
 * @param {string} [text]
 * @returns {St.Label}
 */
export function label(styleClass, text = "") {
  const result = new St.Label({
    style_class: styleClass,
    text,
    y_align: Clutter.ActorAlign.CENTER,
  });
  result.clutter_text.ellipsize = Pango.EllipsizeMode.END;
  return result;
}

/** A group heading: the state icon and `Running · 2`. */
export class GroupHeading {
  actor;
  #label;

  /** @param {JobGroup} group */
  constructor(group) {
    this.actor = new St.BoxLayout({
      style_class: `allot-group-heading allot-group-${group.kind}`,
    });
    this.actor.add_child(
      new St.Icon({ style_class: "allot-group-icon", icon_name: GROUP_ICONS[group.kind] }),
    );
    this.#label = label("allot-group-label", group.heading);
    this.actor.add_child(this.#label);
  }

  /** @param {JobGroup} group */
  update(group) {
    this.#label.text = group.heading;
  }
}

/** One job's card: the session strip, the job row, the stepper and the edge bar. */
export class JobCardView {
  #actions;
  /** @type {JobRow | null} */
  #row = null;

  actor;
  #strip;
  #shortId;
  #project;
  #usage;
  #tile;
  #tileLabel;
  #tileIcon;
  #command;
  #chip;
  #chipIcon;
  #chipLabel;
  #action;
  #detailIcon;
  #detail;
  #time;
  #stepper;
  #bar;

  /**
   * @param {CardActions} actions
   */
  constructor(actions) {
    this.#actions = actions;
    this.actor = new St.BoxLayout({
      style_class: "allot-card",
      orientation: Clutter.Orientation.VERTICAL,
      x_expand: true,
    });

    this.#shortId = label("allot-strip-id");
    this.#project = label("allot-strip-project");
    this.#project.x_expand = true;
    this.#usage = label("allot-strip-usage");
    this.#strip = new St.BoxLayout({ style_class: "allot-strip" });
    for (const child of [this.#shortId, this.#project, this.#usage]) {
      this.#strip.add_child(child);
    }

    this.#tileLabel = new St.Label({ style_class: "allot-tile-label" });
    this.#tileIcon = new St.Icon({ style_class: "allot-tile-icon", visible: false });
    const tileBox = new St.Widget({ layout_manager: new Clutter.BinLayout() });
    tileBox.add_child(this.#tileLabel);
    tileBox.add_child(this.#tileIcon);
    this.#tile = new St.Bin({
      style_class: "allot-tile",
      child: tileBox,
      y_align: Clutter.ActorAlign.CENTER,
    });
    this.#tileLabel.x_align = Clutter.ActorAlign.CENTER;
    this.#tileLabel.y_align = Clutter.ActorAlign.CENTER;
    this.#tileIcon.x_align = Clutter.ActorAlign.CENTER;
    this.#tileIcon.y_align = Clutter.ActorAlign.CENTER;

    this.#command = label("allot-command");
    this.#command.x_expand = true;

    this.#chipIcon = new St.Icon({ style_class: "allot-chip-icon", icon_name: "alarm-symbolic" });
    this.#chipLabel = new St.Label({
      style_class: "allot-chip-label",
      y_align: Clutter.ActorAlign.CENTER,
    });
    this.#chip = new St.BoxLayout({
      style_class: "allot-chip",
      y_align: Clutter.ActorAlign.CENTER,
    });
    this.#chip.add_child(this.#chipIcon);
    this.#chip.add_child(this.#chipLabel);

    this.#action = new St.Button({
      style_class: "allot-action",
      can_focus: true,
      y_align: Clutter.ActorAlign.CENTER,
    });
    this.#action.connect("clicked", () => this.#pressed());
    this.#action.connect("key-focus-in", () => {
      this.#describeAction();
      this.#actions.focused(this.actor);
    });
    this.#action.connect("key-focus-out", () => {
      if (this.#row?.card.action === "confirm") {
        this.#actions.cancel(this.#row.id);
      }
    });

    const top = new St.BoxLayout({ style_class: "allot-row-top" });
    for (const child of [this.#tile, this.#command, this.#chip, this.#action]) {
      top.add_child(child);
    }

    this.#detailIcon = new St.Icon({ style_class: "allot-detail-icon", visible: false });
    this.#detail = new St.Label({ style_class: "allot-detail", x_expand: true });
    this.#time = new St.Label({ style_class: "allot-time" });
    const detailLine = new St.BoxLayout({ style_class: "allot-detail-line" });
    for (const child of [this.#detailIcon, this.#detail, this.#time]) {
      detailLine.add_child(child);
    }

    this.#stepper = new Stepper();

    const row = new St.BoxLayout({
      style_class: "allot-row",
      orientation: Clutter.Orientation.VERTICAL,
    });
    row.add_child(top);
    row.add_child(detailLine);
    row.add_child(this.#stepper.actor);

    this.#bar = new EdgeBar();

    this.actor.add_child(this.#strip);
    this.actor.add_child(row);
    this.actor.add_child(this.#bar.actor);
  }

  /** @returns {St.Button} */
  get actionButton() {
    return this.#action;
  }

  /**
   * @param {JobRow} row
   * @param {boolean} animate Ease the bar and stepper to their new values.
   */
  update(row, animate) {
    const before = this.#row;
    this.#row = row;
    const { card, session } = row;

    this.#shortId.text = session.shortId;
    this.#project.text = session.project;
    this.#usage.text = session.usage;
    this.#usage.visible = session.usage !== "";

    this.#showTile(card);
    this.#command.accessible_name = row.command;
    this.#command.clutter_text.set_markup(
      card.isStruck ? `<s>${escape(row.command)}</s>` : escape(row.command),
    );
    this.#chipLabel.text = card.chip;
    this.#chipIcon.visible = card.isBenchmark;
    setClass(this.#chip, "allot-chip-benchmark", card.isBenchmark);
    for (const actor of [this.#tile, this.#command, this.#chip]) {
      actor.opacity = card.isEnded ? dimmedOpacity(ENDED_OPACITY) : 255;
    }

    this.#showAction(card, before?.card.action ?? "none");
    this.#showDetail(card);
    this.#stepper.update(card.steps, animate);
    this.#bar.update(card, animate && before !== null);
  }

  /** @param {JobCard} card */
  #showTile(card) {
    const icon = card.isQueued ? null : this.#actions.appIcon(card.exePath);
    setClass(this.#tile, "allot-tile-queued", card.isQueued);
    this.#tileIcon.visible = icon !== null;
    this.#tileLabel.visible = icon === null;
    if (icon !== null) {
      this.#tileIcon.gicon = icon;
    }
    this.#tileLabel.text = card.tile;
  }

  /**
   * @param {JobCard} card
   * @param {JobCard["action"]} previous
   */
  #showAction(card, previous) {
    const button = this.#action;
    for (const kind of ["kill", "confirm", "remove", "none"]) {
      setClass(button, `allot-action-${kind}`, card.action === kind);
    }
    button.accessible_name = card.actionName;
    button.reactive = card.action !== "none";
    button.can_focus = card.action !== "none";
    if (card.action === previous && button.get_child() !== null) {
      return;
    }
    if (button.has_key_focus()) {
      this.#describeAction();
    }
    switch (card.action) {
      case "kill":
        button.child = new St.Icon({ icon_name: "process-stop-symbolic" });
        break;
      case "confirm": {
        const box = new St.BoxLayout({ style_class: "allot-confirm" });
        box.add_child(new St.Icon({ icon_name: "process-stop-symbolic" }));
        box.add_child(new St.Label({ text: "Kill?", y_align: Clutter.ActorAlign.CENTER }));
        button.child = box;
        // The handoff moves focus to the confirm, so Enter or Space can give the second press.
        if (previous !== "confirm") {
          button.grab_key_focus();
        }
        break;
      }
      case "remove":
        button.child = new St.Label({ text: "Remove", y_align: Clutter.ActorAlign.CENTER });
        break;
      case "none":
        button.child = new St.Widget();
        break;
    }
  }

  /** @param {JobCard} card */
  #showDetail(card) {
    const icons = { timer: "alarm-symbolic", check: "object-select-symbolic" };
    this.#detailIcon.visible = card.icon !== "none";
    if (card.icon !== "none") {
      this.#detailIcon.icon_name = icons[card.icon];
    }
    setClass(this.#detailIcon, "allot-detail-icon-check", card.icon === "check");
    const strong = card.strong === "" ? "" : `<b>${escape(card.strong)}</b>`;
    this.#detail.clutter_text.set_markup(strong + escape(card.detail));
    // A line with an icon is a benchmark status, which wraps instead of truncating.
    const wraps = card.icon !== "none";
    this.#detail.clutter_text.line_wrap = wraps;
    this.#detail.clutter_text.ellipsize = wraps
      ? Pango.EllipsizeMode.NONE
      : Pango.EllipsizeMode.END;
    if (this.#row !== null) {
      this.#detail.accessible_name = detailText(this.#row);
    }
    setClass(this.#detail, "allot-danger", card.detailIsDanger);
    this.#time.text = card.time;
    this.#time.visible = card.time !== "";
    setClass(this.#time, "allot-warning", card.timeIsWarning);
  }

  // Only when focus arrives or the action changes: the text holds the ticking time, and Orca
  // reads out every change to the focused control's description.
  #describeAction() {
    if (this.#row !== null) {
      this.#action.get_accessible()?.set_description(cardDescription(this.#row));
    }
  }

  #pressed() {
    const row = this.#row;
    if (row === null) {
      return;
    }
    switch (row.card.action) {
      case "kill":
      case "confirm":
        this.#actions.kill(row.id);
        break;
      case "remove":
        this.#actions.remove(row.id);
        break;
      case "none":
        break;
    }
  }
}

/**
 * The 2 px bar along a card's bottom. St does not clip children to a parent's rounded corners, so
 * the track and the fill are drawn together, cut to the card's bottom curve.
 */
class EdgeBar {
  actor;
  #share;
  #isDashed = false;

  constructor() {
    this.actor = new St.DrawingArea({ style_class: "allot-bar", x_expand: true });
    this.actor.connect("repaint", () => this.#paint());
    // The share eases through an adjustment, since a drawing has no animatable property for it.
    this.#share = new St.Adjustment({ actor: this.actor, lower: 0, upper: 1, value: 0 });
    // Tied to the drawing: a bar still easing when it is destroyed must not repaint.
    this.#share.connectObject("notify::value", () => this.actor.queue_repaint(), this.actor);
  }

  /**
   * @param {JobCard} card
   * @param {boolean} animate
   */
  update(card, animate) {
    this.actor.visible = card.barTone !== "none";
    for (const tone of ["running", "over", "waiting", "waiting-dashed", "frozen"]) {
      setClass(this.actor, `allot-bar-${tone}`, card.barTone === tone);
    }
    this.#isDashed = card.barTone === "waiting-dashed";
    const share = this.#isDashed ? 1 : card.barFraction;
    if (animate && card.barTone !== "frozen") {
      this.#share.ease(share, { duration: BAR_MS, mode: Clutter.AnimationMode.LINEAR });
    } else {
      this.#share.remove_transition("value");
      this.#share.value = share;
    }
    this.actor.queue_repaint();
  }

  #paint() {
    const context = this.actor.get_context();
    const [width, height] = this.actor.get_surface_size();
    const node = this.actor.get_theme_node();
    const radius = node.get_length("-allot-bar-radius");

    // The card's inner bottom edge; its corners reach above this strip.
    context.moveTo(0, -radius);
    context.lineTo(width, -radius);
    context.arc(width - radius, height - radius, radius, 0, Math.PI / 2);
    context.arc(radius, height - radius, radius, Math.PI / 2, Math.PI);
    context.closePath();
    context.clip();

    // While the shell swaps stylesheets on a light or dark change, the track has no colour.
    const [hasTrack, track] = node.lookup_color("-allot-track-color", false);
    if (hasTrack) {
      setSource(context, track);
      context.paint();
    }
    setSource(context, node.get_foreground_color());
    if (this.#isDashed) {
      const dash = DASH_PX * St.ThemeContext.get_for_stage(global.stage).scale_factor;
      for (let x = 0; x < width; x += dash * 2) {
        context.rectangle(x, 0, dash, height);
      }
    } else {
      context.rectangle(0, 0, width * this.#share.value, height);
    }
    context.fill();
    context.$dispose();
  }
}

/** The Drain / Quiet / Run stepper of a benchmark card. */
class Stepper {
  actor;
  #segments;
  #ticks;
  #runFill;

  constructor() {
    this.actor = new St.BoxLayout({ style_class: "allot-stepper", visible: false });
    this.#ticks = [];
    const quietBar = new St.BoxLayout({ style_class: "allot-step-ticks" });
    for (let index = 0; index < QUIET_TICKS; index++) {
      const tick = new St.Widget({ style_class: "allot-step-bar allot-step-tick", x_expand: true });
      this.#ticks.push(tick);
      quietBar.add_child(tick);
    }
    this.#runFill = new St.Widget({ style_class: "allot-step-run-fill" });
    const runTrack = new St.Widget({ style_class: "allot-step-bar" });
    runTrack.add_child(this.#runFill);
    spanWidth(this.#runFill, runTrack);
    this.#segments = {
      drain: segment(
        "allot-step-drain",
        "Drain",
        new St.Widget({ style_class: "allot-step-bar allot-step-solid" }),
      ),
      quiet: segment("allot-step-quiet", "Quiet", quietBar),
      run: segment("allot-step-run", "Run", runTrack),
    };
    for (const part of Object.values(this.#segments)) {
      this.actor.add_child(part.box);
    }
  }

  /**
   * @param {JobCard["steps"]} steps
   * @param {boolean} animate
   */
  update(steps, animate) {
    this.actor.visible = steps !== null;
    if (steps === null) {
      return;
    }
    const order = ["drain", "quiet", "run", "done"];
    const current = order.indexOf(steps.current);
    for (const [name, part] of Object.entries(this.#segments)) {
      const index = order.indexOf(name);
      const state = index < current ? "past" : index === current ? "current" : "upcoming";
      for (const stateName of ["past", "current", "upcoming"]) {
        setClass(part.box, `allot-step-${stateName}`, state === stateName);
      }
    }
    this.#ticks.forEach((tick, index) => setClass(tick, "allot-step-on", index < steps.quietTicks));
    fillTo(this.#runFill, steps.runFraction, animate ? STEP_RUN_MS : 0);
  }
}

/**
 * @param {string} styleClass
 * @param {string} name
 * @param {St.Widget} bar
 * @returns {{ box: St.BoxLayout }}
 */
function segment(styleClass, name, bar) {
  const box = new St.BoxLayout({
    style_class: `allot-step ${styleClass}`,
    orientation: Clutter.Orientation.VERTICAL,
  });
  box.add_child(bar);
  box.add_child(new St.Label({ style_class: "allot-step-label", text: name }));
  return { box };
}

/** The Recently finished heading and its card of rows. */
export class RecentView {
  actor;
  #card;

  constructor() {
    this.actor = new St.BoxLayout({
      style_class: "allot-recent-view",
      orientation: Clutter.Orientation.VERTICAL,
    });
    const heading = new St.BoxLayout({ style_class: "allot-group-heading allot-group-recent" });
    heading.add_child(
      new St.Icon({ style_class: "allot-group-icon", icon_name: "object-select-symbolic" }),
    );
    heading.add_child(label("allot-group-label", "Recently finished"));
    this.#card = new St.BoxLayout({
      style_class: "allot-card allot-recent",
      orientation: Clutter.Orientation.VERTICAL,
    });
    this.actor.add_child(heading);
    this.actor.add_child(this.#card);
  }

  /** @param {RecentLine[]} lines */
  update(lines) {
    this.#card.destroy_all_children();
    for (const line of lines) {
      const row = new St.BoxLayout({ style_class: "allot-recent-row" });
      const command = label("allot-command", line.command);
      command.x_expand = true;
      row.add_child(command);
      if (line.shortId !== "") {
        row.add_child(label("allot-recent-id", line.shortId));
      }
      row.add_child(label("allot-recent-meta", line.meta));
      this.#card.add_child(row);
    }
  }
}

/** The first-run card: what Allot does, whether banners are hidden, and Done. */
export class WelcomeView {
  actor;
  #doNotDisturb;

  /** @param {() => void} onDone */
  constructor(onDone) {
    this.actor = new St.BoxLayout({
      style_class: "allot-card allot-welcome",
      orientation: Clutter.Orientation.VERTICAL,
      x_expand: true,
    });
    this.actor.add_child(label("allot-welcome-title", WELCOME_TITLE));
    this.actor.add_child(wrapped("allot-welcome-body", WELCOME_BODY));
    this.#doNotDisturb = wrapped("allot-welcome-note", DO_NOT_DISTURB_TEXT);
    this.actor.add_child(this.#doNotDisturb);
    const done = new St.Button({
      style_class: "button allot-welcome-done",
      label: "Done",
      can_focus: true,
      x_align: Clutter.ActorAlign.END,
    });
    done.connect("clicked", () => onDone());
    this.actor.add_child(done);
  }

  /** @param {WelcomeCard} welcome */
  update(welcome) {
    this.#doNotDisturb.visible = welcome.doNotDisturb;
  }
}

/**
 * The red banner and the caption above the last known cards. The banner stays undimmed, so it
 * lives above the list.
 */
export class ServiceBannerView {
  actor;
  #text;
  #problem;
  #button;
  #caption;
  #banner;

  /** @param {() => void} onStart Start Service was pressed. */
  constructor(onStart) {
    this.#text = wrapped("allot-banner-text", "");
    this.#problem = wrapped("allot-banner-problem", "");
    const lines = new St.BoxLayout({
      orientation: Clutter.Orientation.VERTICAL,
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });
    lines.add_child(this.#text);
    lines.add_child(this.#problem);
    this.#button = new St.Button({
      style_class: "button allot-banner-button",
      can_focus: true,
      y_align: Clutter.ActorAlign.CENTER,
    });
    this.#button.connect("clicked", () => onStart());
    this.#banner = new St.BoxLayout({ style_class: "allot-banner" });
    this.#banner.add_child(lines);
    this.#banner.add_child(this.#button);
    this.#caption = label("allot-banner-caption");
    this.actor = new St.BoxLayout({
      style_class: "allot-banner-view",
      orientation: Clutter.Orientation.VERTICAL,
      clip_to_allocation: true,
      visible: false,
    });
    this.actor.add_child(this.#banner);
    this.actor.add_child(this.#caption);
  }

  /**
   * @param {ServiceBanner | null} banner
   * @param {boolean} animate
   */
  update(banner, animate) {
    if (banner !== null) {
      this.#text.text = banner.text;
      this.#problem.text = banner.problem;
      this.#problem.visible = banner.problem !== "";
      this.#button.visible = banner.button !== "none";
      this.#button.reactive = banner.button === "start";
      this.#button.can_focus = banner.button === "start";
      this.#button.label = banner.button === "starting" ? "Starting…" : "Start Service";
      setClass(this.#button, "allot-banner-button-muted", banner.button === "starting");
      this.#caption.text = banner.caption;
      this.#caption.visible = banner.caption !== "";
    }
    reveal(this.actor, banner !== null, animate);
  }
}

/**
 * Fades a new row in.
 *
 * @param {Clutter.Actor} actor
 */
export function fadeIn(actor) {
  actor.opacity = 0;
  actor.ease({ opacity: 255, duration: FADE_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD });
}

/**
 * Collapses a leaving row's height and opacity, then destroys it.
 *
 * @param {Clutter.Actor} actor
 */
export function collapse(actor) {
  actor.reactive = false;
  actor.set_height(actor.height);
  actor.ease({
    opacity: 0,
    height: 0,
    duration: FADE_MS,
    mode: Clutter.AnimationMode.EASE_OUT_QUAD,
    onStopped: () => actor.destroy(),
  });
}

/**
 * Shows or hides a block that takes its own height, easing that height and its opacity. The block
 * clips its children, so they are cut rather than squeezed on the way.
 *
 * @param {St.Widget} actor
 * @param {boolean} show
 * @param {boolean} animate
 */
export function reveal(actor, show, animate) {
  if (actor.allotRevealed === show) {
    return;
  }
  actor.allotRevealed = show;
  actor.remove_all_transitions();
  if (!animate) {
    actor.visible = show;
    actor.opacity = 255;
    actor.height = -1;
    return;
  }
  const mode = Clutter.AnimationMode.EASE_OUT_QUAD;
  if (show) {
    actor.show();
    const width = actor.get_parent()?.width ?? -1;
    const [, natural] = actor.get_preferred_height(width);
    actor.opacity = 0;
    actor.height = 0;
    actor.ease({
      opacity: 255,
      height: natural,
      duration: FADE_MS,
      mode,
      onComplete: () => {
        actor.height = -1;
      },
    });
  } else {
    actor.set_height(actor.height);
    actor.ease({
      opacity: 0,
      height: 0,
      duration: FADE_MS,
      mode,
      onComplete: () => {
        actor.hide();
        actor.height = -1;
        actor.opacity = 255;
      },
    });
  }
}

/**
 * @param {string} styleClass
 * @param {string} text
 * @returns {St.Label} A label that wraps instead of truncating.
 */
function wrapped(styleClass, text) {
  const result = new St.Label({ style_class: styleClass, text });
  result.clutter_text.line_wrap = true;
  result.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
  return result;
}

/**
 * Makes a bar's fill as wide as its track and anchors it at the left. The filled share is then
 * only the fill's horizontal scale, which eases without a relayout on every tick.
 *
 * @param {Clutter.Actor} fill
 * @param {Clutter.Actor} track
 */
function spanWidth(fill, track) {
  fill.add_constraint(
    new Clutter.BindConstraint({ source: track, coordinate: Clutter.BindCoordinate.WIDTH }),
  );
  fill.set_pivot_point(0, 0.5);
  fill.scale_x = 0;
}

/**
 * @param {Clutter.Actor} fill
 * @param {number} fraction From 0 to 1.
 * @param {number} duration Milliseconds; 0 jumps there.
 */
function fillTo(fill, fraction, duration) {
  fill.remove_transition("scale-x");
  if (duration === 0) {
    fill.scale_x = fraction;
  } else {
    fill.ease({ scale_x: fraction, duration, mode: Clutter.AnimationMode.LINEAR });
  }
}

/**
 * @param {import("gi://cairo").Context} context
 * @param {import("gi://Cogl").Color} color
 */
export function setSource(context, color) {
  context.setSourceRGBA(color.red / 255, color.green / 255, color.blue / 255, color.alpha / 255);
}

/**
 * @param {St.Widget} actor
 * @param {string} name
 * @param {boolean} on
 */
export function setClass(actor, name, on) {
  if (on) {
    actor.add_style_class_name(name);
  } else {
    actor.remove_style_class_name(name);
  }
}

/**
 * @param {string} text
 * @returns {string}
 */
function escape(text) {
  return GLib.markup_escape_text(text, -1);
}
