import Atk from "gi://Atk";
import Clutter from "gi://Clutter";
import Gio from "gi://Gio";
import GLib from "gi://GLib";
import St from "gi://St";

import * as Main from "resource:///org/gnome/shell/ui/main.js";

import { BarShare } from "./bar-share.js";
import { label, setSource } from "./cards.js";
import { driveDescription, driveName, meterColumns, meterName } from "./lib/accessible-text.js";

/** @import { BarGlide } from "./bar-share.js" */
/** @import { DriveRow, MeterRow, ResourcesView } from "./lib/resources-view.js" */

const FOLD_MS = 240;
const SUMMARY_MS = 150;
const GLIDE_MS = 600;
const HOVER_DELAY_MS = 300;
const HOVER_GAP_PX = 4;
const BAR_RADIUS_PX = 2;
const HATCH_STEP_PX = 4;
const HATCH_LINE_PX = 1.5;
const BAR_HEIGHT_PX = 6;
const TICK_WIDTH_PX = 2;
const TICK_HEIGHT_PX = 14;
const SPARK_STROKE_PX = 1.4;
// One line: the fold measures its rows without a width, so a wrapped line would be cut.
const ALL_OFF_TEXT = "All meters are off. Turn them on from ⋯ › Meters.";
const INTERFACE_SCHEMA = "org.gnome.desktop.interface";
const TEXT_SCALE = "text-scaling-factor";

/**
 * The docked Resources card: a header that folds the rows open and shut, and the rows themselves.
 * Rows stay laid out inside a clip while it folds, so nothing squeezes on the way.
 */
export class ResourcesCard {
  actor;
  #onToggle;
  #onHeightChanged;
  #hover = new HoverLabel();

  #summary;
  #chevron;
  #header;
  #fold;
  #body;
  #meters;
  #separator;
  #drives;
  #allOff;
  #legend;
  #sharedLegend;
  /** @type {Map<string, MeterRowView>} */
  #meterViews = new Map();
  /** @type {Map<string, DriveRowView>} */
  #driveViews = new Map();
  /** @type {boolean | null} */
  #isExpanded = null;
  #interfaceSettings = new Gio.Settings({ schema_id: INTERFACE_SCHEMA });
  #textScaleId;
  /** @type {import("./lib/accessible-text.js").MeterColumns} */
  #columns;

  /**
   * @param {object} options
   * @param {() => void} options.onToggle The header was pressed.
   * @param {() => void} options.onHeightChanged The card folded a step, or finished folding.
   */
  constructor(options) {
    this.#onToggle = options.onToggle;
    this.#onHeightChanged = options.onHeightChanged;
    this.#columns = this.#readColumns();
    this.#textScaleId = this.#interfaceSettings.connect(`changed::${TEXT_SCALE}`, () => {
      this.#columns = this.#readColumns();
      for (const view of [...this.#meterViews.values(), ...this.#driveViews.values()]) {
        view.setColumns(this.#columns);
      }
    });

    this.#summary = label("allot-resources-summary");
    this.#summary.x_expand = true;
    this.#chevron = new St.Icon({ style_class: "allot-resources-chevron" });
    const headerBox = new St.BoxLayout({ x_expand: true });
    headerBox.add_child(label("allot-resources-title", "Resources"));
    headerBox.add_child(this.#summary);
    headerBox.add_child(this.#chevron);
    this.#header = new St.Button({
      style_class: "allot-resources-header",
      can_focus: true,
      x_expand: true,
      child: headerBox,
    });
    this.#header.add_accessible_state(Atk.StateType.EXPANDABLE);
    this.#header.connect("clicked", () => this.#onToggle());

    this.#meters = new St.BoxLayout({ orientation: Clutter.Orientation.VERTICAL });
    this.#separator = new St.Widget({ style_class: "allot-resources-separator" });
    this.#drives = new St.BoxLayout({ orientation: Clutter.Orientation.VERTICAL });
    this.#allOff = label("allot-resources-off", ALL_OFF_TEXT);
    this.#legend = new St.BoxLayout({ style_class: "allot-legend" });
    this.#sharedLegend = legendEntry("allot-legend-shared", "GPU");
    this.#legend.add_child(legendEntry("allot-legend-used", "Used"));
    this.#legend.add_child(this.#sharedLegend);
    this.#legend.add_child(legendEntry("allot-legend-reserved", "Reserved"));
    this.#legend.add_child(hatchLegendEntry());

    this.#body = new St.BoxLayout({
      style_class: "allot-resources-body",
      orientation: Clutter.Orientation.VERTICAL,
    });
    for (const child of [this.#meters, this.#separator, this.#drives, this.#allOff, this.#legend]) {
      this.#body.add_child(child);
    }

    // A fixed layout gives the body its full height whatever the clip's height is.
    this.#fold = new St.Widget({
      layout_manager: new Clutter.FixedLayout(),
      clip_to_allocation: true,
      x_expand: true,
      height: 0,
    });
    this.#fold.add_child(this.#body);
    this.#body.add_constraint(
      new Clutter.BindConstraint({ source: this.#fold, coordinate: Clutter.BindCoordinate.WIDTH }),
    );
    this.#fold.connect("notify::height", () => this.#onHeightChanged());

    this.actor = new St.BoxLayout({
      style_class: "allot-card allot-resources",
      orientation: Clutter.Orientation.VERTICAL,
      x_expand: true,
      visible: false,
    });
    this.actor.add_child(this.#header);
    this.actor.add_child(this.#fold);
  }

  /**
   * @param {ResourcesView} view
   * @param {boolean} animate Fold and glide; off when the popup opens.
   */
  update(view, animate) {
    this.actor.visible = view.isVisible;
    if (!view.isVisible) {
      this.#hover.hide();
      return;
    }
    this.#showMeters(view.meters, animate);
    this.#showDrives(view.drives, animate);
    this.#separator.visible = view.showsSeparator;
    this.#allOff.visible = view.allOff;
    this.#legend.visible = view.showsLegend;
    this.#sharedLegend.visible = view.showsSharedLegend;
    this.#showHeader(view);
    if (view.isExpanded !== this.#isExpanded) {
      this.#isExpanded = view.isExpanded;
      this.#foldTo(view.isExpanded, animate);
    }
  }

  /** The popup closed: a hover label must not stay behind. */
  hideHover() {
    this.#hover.hide();
  }

  destroy() {
    this.#interfaceSettings.disconnect(this.#textScaleId);
    this.#hover.destroy();
    this.actor.destroy();
  }

  /** @returns {import("./lib/accessible-text.js").MeterColumns} */
  #readColumns() {
    return meterColumns(this.#interfaceSettings.get_double(TEXT_SCALE));
  }

  /** @param {ResourcesView} view */
  #showHeader(view) {
    // The text stays while it fades out, since a collapsed card's summary is what it fades.
    if (view.summary !== "") {
      this.#summary.text = view.summary;
    }
    this.#chevron.icon_name = view.isExpanded ? "pan-down-symbolic" : "pan-up-symbolic";
    this.#header.accessible_name = view.isExpanded ? "Collapse resources" : "Expand resources";
    if (view.isExpanded) {
      this.#header.add_accessible_state(Atk.StateType.EXPANDED);
    } else {
      this.#header.remove_accessible_state(Atk.StateType.EXPANDED);
    }
  }

  /**
   * @param {boolean} expanded
   * @param {boolean} animate
   */
  #foldTo(expanded, animate) {
    const [, natural] = this.#body.get_preferred_height(-1);
    const height = expanded ? natural : 0;
    this.#fold.remove_transition("height");
    this.#summary.remove_transition("opacity");
    const summaryOpacity = expanded ? 0 : 255;
    if (!animate) {
      this.#fold.height = expanded ? -1 : 0;
      this.#summary.opacity = summaryOpacity;
      this.#onHeightChanged();
      return;
    }
    this.#summary.ease({
      opacity: summaryOpacity,
      duration: SUMMARY_MS,
      mode: Clutter.AnimationMode.EASE_OUT_QUAD,
    });
    this.#fold.ease({
      height,
      duration: FOLD_MS,
      mode: Clutter.AnimationMode.EASE_OUT_QUAD,
      onComplete: () => {
        // Open, the card follows its rows again when a meter is turned on or off.
        if (expanded) {
          this.#fold.height = -1;
        }
        this.#onHeightChanged();
      },
    });
  }

  /**
   * @param {MeterRow[]} rows
   * @param {boolean} animate
   */
  #showMeters(rows, animate) {
    this.#showRows(
      rows,
      this.#meterViews,
      this.#meters,
      () => new MeterRowView(this.#hover),
      animate,
    );
  }

  /**
   * @param {DriveRow[]} rows
   * @param {boolean} animate
   */
  #showDrives(rows, animate) {
    this.#showRows(
      rows,
      this.#driveViews,
      this.#drives,
      () => new DriveRowView(this.#hover),
      animate,
    );
  }

  /**
   * Keeps one view per row id, in the rows' order.
   *
   * @template {{ id: string }} Row
   * @template {RowView<Row>} View
   * @param {Row[]} rows
   * @param {Map<string, View>} views
   * @param {St.BoxLayout} box
   * @param {() => View} make
   * @param {boolean} animate
   */
  #showRows(rows, views, box, make, animate) {
    const shown = new Set(rows.map((row) => row.id));
    for (const [id, view] of [...views]) {
      if (!shown.has(id)) {
        views.delete(id);
        view.actor.destroy();
      }
    }
    rows.forEach((row, index) => {
      let view = views.get(row.id);
      const isNew = view === undefined;
      if (view === undefined) {
        view = make();
        view.setColumns(this.#columns);
        views.set(row.id, view);
        box.add_child(view.actor);
      }
      box.set_child_at_index(view.actor, index);
      view.update(row, animate && !isNew);
    });
  }
}

/**
 * The name and description a row gives assistive tech. The description is set only when it
 * changes, since each set tells every listening screen reader.
 *
 * @param {St.Widget} actor
 * @param {string} name
 * @param {string} description
 */
function describeRow(actor, name, description) {
  actor.accessible_name = name;
  if (actor.allotDescription !== description) {
    actor.allotDescription = description;
    actor.get_accessible()?.set_description(description);
  }
}

/**
 * One row's view, kept by row id.
 *
 * @template Row
 * @typedef {object} RowView
 * @property {Clutter.Actor} actor
 * @property {(row: Row, animate: boolean) => void} update
 * @property {(columns: import("./lib/accessible-text.js").MeterColumns) => void} setColumns
 */

/**
 * @param {St.Widget} label
 * @param {St.Widget} value
 * @param {import("./lib/accessible-text.js").MeterColumns} columns
 */
function showColumns(label, value, columns) {
  label.style = `width: ${columns.label}px;`;
  value.style = `width: ${columns.value}px;`;
}

/** One bullet-bar row: label, bar and value, with the detail in a hover label. */
class MeterRowView {
  actor;
  #label;
  #bar;
  #spark;
  #value;
  #valueBox;

  /** @param {HoverLabel} hover */
  constructor(hover) {
    this.#label = label("allot-meter-label");
    this.#bar = new BulletBar();
    this.#spark = new Sparkline();
    this.#value = label("allot-meter-value");
    this.#value.x_expand = true;
    this.#value.x_align = Clutter.ActorAlign.END;
    this.#valueBox = new St.BoxLayout({ style_class: "allot-meter-value-box" });
    this.#valueBox.add_child(this.#spark.actor);
    this.#valueBox.add_child(this.#value);
    this.actor = hover.attach(new St.BoxLayout({ style_class: "allot-meter-row" }));
    this.actor.add_child(this.#label);
    this.actor.add_child(this.#bar.actor);
    this.actor.add_child(this.#valueBox);
  }

  /** @param {import("./lib/accessible-text.js").MeterColumns} columns */
  setColumns(columns) {
    showColumns(this.#label, this.#valueBox, columns);
  }

  /**
   * @param {MeterRow} row
   * @param {boolean} animate
   */
  update(row, animate) {
    this.#label.text = row.label;
    this.#value.text = row.value;
    this.actor.allotHover = row.detail;
    describeRow(this.actor, meterName(row), row.detail);
    this.#bar.update(row, animate);
    this.#spark.update(row.spark);
  }
}

/** One drive: label, usage bar with the read and write rates, and space. */
class DriveRowView {
  actor;
  #label;
  #bar;
  #activity;
  #value;
  #valueBox;

  /** @param {HoverLabel} hover */
  constructor(hover) {
    this.#label = label("allot-meter-label");
    this.#bar = new DriveBar();
    this.#activity = label("allot-drive-activity");
    this.#value = label("allot-meter-value");
    this.#value.x_expand = true;
    this.#value.x_align = Clutter.ActorAlign.END;
    this.#valueBox = new St.BoxLayout({ style_class: "allot-meter-value-box" });
    this.#valueBox.add_child(this.#value);
    const middle = new St.BoxLayout({
      style_class: "allot-drive-middle",
      orientation: Clutter.Orientation.VERTICAL,
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });
    middle.add_child(this.#bar.actor);
    middle.add_child(this.#activity);
    this.actor = hover.attach(new St.BoxLayout({ style_class: "allot-meter-row" }));
    this.actor.add_child(this.#label);
    this.actor.add_child(middle);
    this.actor.add_child(this.#valueBox);
  }

  /** @param {import("./lib/accessible-text.js").MeterColumns} columns */
  setColumns(columns) {
    showColumns(this.#label, this.#valueBox, columns);
  }

  /**
   * @param {DriveRow} row
   * @param {boolean} animate
   */
  update(row, animate) {
    this.#label.text = row.label;
    this.#value.text = row.value;
    this.#activity.text = row.activity;
    this.#activity.visible = row.activity !== "";
    this.actor.allotHover = row.id;
    describeRow(this.actor, driveName(row), driveDescription(row));
    this.#bar.update(row.used, animate);
  }
}

/**
 * @param {boolean} animate
 * @returns {BarGlide | null}
 */
function glideIf(animate) {
  return animate ? { duration: GLIDE_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD } : null;
}

/**
 * The handoff's bullet bar: a track whose last part is hatched headroom, the used fill, an
 * optional lighter part at the fill's end for shared GPU memory, and the reserved tick.
 */
class BulletBar {
  actor;
  #used;
  #shared;
  /** @type {MeterRow | null} */
  #row = null;

  constructor() {
    this.actor = new St.DrawingArea({ style_class: "allot-meter-bar", x_expand: true });
    this.actor.connect("repaint", () => this.#paint());
    this.#used = new BarShare(this.actor);
    this.#shared = new BarShare(this.actor);
  }

  /**
   * @param {MeterRow} row
   * @param {boolean} animate
   */
  update(row, animate) {
    const before = this.#row;
    this.#row = row;
    this.#used.moveTo(row.used, glideIf(animate));
    this.#shared.moveTo(row.shared ?? 0, glideIf(animate));
    if (before?.headroom !== row.headroom || before?.reserved !== row.reserved) {
      this.actor.queue_repaint();
    }
  }

  #paint() {
    const context = this.actor.get_context();
    const [width, height] = this.actor.get_surface_size();
    const row = this.#row;
    const node = this.actor.get_theme_node();
    // While the shell swaps stylesheets on a light or dark change, the custom colours are gone.
    const [hasTrack, track] = node.lookup_color("-allot-track-color", false);
    const [hasHatch, hatch] = node.lookup_color("-allot-hatch-color", false);
    const [hasTick, tick] = node.lookup_color("-allot-tick-color", false);
    const [hasRing, ring] = node.lookup_color("-allot-ring-color", false);
    const [hasTint, tint] = node.lookup_color("-allot-shared-color", false);
    if (row === null || !hasTrack || !hasHatch || !hasTick || !hasRing || !hasTint) {
      context.$dispose();
      return;
    }

    // The surface is in device pixels, so at 200% these sizes must double like the CSS ones do.
    const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
    const barHeight = BAR_HEIGHT_PX * scale;
    const top = (height - barHeight) / 2;
    const headroomX = width * row.headroom;

    context.save();
    roundedRectangle(context, 0, top, width, barHeight, BAR_RADIUS_PX * scale);
    context.clip();
    setSource(context, track);
    context.rectangle(0, top, headroomX, barHeight);
    context.fill();
    paintHatch(context, hatch, headroomX, top, width - headroomX, barHeight);
    const usedWidth = width * this.#used.value;
    setSource(context, node.get_foreground_color());
    context.rectangle(0, top, usedWidth, barHeight);
    context.fill();
    if (this.#shared.value > 0) {
      const sharedWidth = Math.min(usedWidth, width * this.#shared.value);
      setSource(context, tint);
      context.rectangle(usedWidth - sharedWidth, top, sharedWidth, barHeight);
      context.fill();
    }
    context.restore();

    if (row.reserved !== null) {
      const tickWidth = TICK_WIDTH_PX * scale;
      const tickHeight = TICK_HEIGHT_PX * scale;
      const x = Math.min(width - tickWidth, width * row.reserved - tickWidth / 2);
      const tickTop = (height - tickHeight) / 2;
      setSource(context, ring);
      context.rectangle(x - scale, tickTop - scale, tickWidth + 2 * scale, tickHeight + 2 * scale);
      context.fill();
      setSource(context, tick);
      context.rectangle(x, tickTop, tickWidth, tickHeight);
      context.fill();
    }
    context.$dispose();
  }
}

/** A drive's thin usage bar. */
class DriveBar {
  actor;
  #used;

  constructor() {
    this.actor = new St.DrawingArea({ style_class: "allot-drive-bar", x_expand: true });
    this.actor.connect("repaint", () => this.#paint());
    this.#used = new BarShare(this.actor);
  }

  /**
   * @param {number} used
   * @param {boolean} animate
   */
  update(used, animate) {
    this.#used.moveTo(used, glideIf(animate));
  }

  #paint() {
    const context = this.actor.get_context();
    const [width, height] = this.actor.get_surface_size();
    const node = this.actor.get_theme_node();
    const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
    roundedRectangle(context, 0, 0, width, height, BAR_RADIUS_PX * scale);
    context.clip();
    const [hasTrack, track] = node.lookup_color("-allot-track-color", false);
    if (hasTrack) {
      setSource(context, track);
      context.paint();
    }
    setSource(context, node.get_foreground_color());
    context.rectangle(0, 0, width * this.#used.value, height);
    context.fill();
    context.$dispose();
  }
}

/** The CPU row's small load history. */
class Sparkline {
  actor;
  /** @type {number[]} */
  #samples = [];

  constructor() {
    this.actor = new St.DrawingArea({
      style_class: "allot-sparkline",
      y_align: Clutter.ActorAlign.CENTER,
      visible: false,
    });
    this.actor.connect("repaint", () => this.#paint());
  }

  /** @param {number[]} samples */
  update(samples) {
    this.#samples = samples;
    this.actor.visible = samples.length > 0;
    this.actor.queue_repaint();
  }

  #paint() {
    const context = this.actor.get_context();
    const [width, height] = this.actor.get_surface_size();
    const samples = this.#samples;
    if (samples.length > 1) {
      setSource(context, this.actor.get_theme_node().get_foreground_color());
      const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
      context.setLineWidth(SPARK_STROKE_PX * scale);
      samples.forEach((sample, index) => {
        const x = (index / (samples.length - 1)) * width;
        const y = height - sample * height;
        if (index === 0) {
          context.moveTo(x, y);
        } else {
          context.lineTo(x, y);
        }
      });
      context.stroke();
    }
    context.$dispose();
  }
}

/**
 * The shell has no tooltips, so rows show their detail in one small label that floats under the
 * hovered row. A row carries its text in `allotHover`, read when the label shows.
 */
class HoverLabel {
  #label;
  /** @type {St.Widget | null} */
  #row = null;
  #timer = 0;

  constructor() {
    this.#label = new St.Label({ style_class: "allot-hover-label", visible: false });
    Main.uiGroup.add_child(this.#label);
  }

  /**
   * @template {St.Widget} Row
   * @param {Row} row
   * @returns {Row}
   */
  attach(row) {
    row.reactive = true;
    row.track_hover = true;
    row.connect("notify::hover", () => {
      if (row.hover) {
        this.#schedule(row);
      } else if (this.#row === row) {
        this.hide();
      }
    });
    row.connect("destroy", () => {
      if (this.#row === row) {
        this.hide();
      }
    });
    return row;
  }

  hide() {
    this.#clearTimer();
    this.#row = null;
    this.#label.hide();
  }

  destroy() {
    this.#clearTimer();
    this.#row = null;
    this.#label.destroy();
  }

  /** @param {St.Widget} row */
  #schedule(row) {
    this.#clearTimer();
    this.#label.hide();
    this.#row = row;
    this.#timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, HOVER_DELAY_MS, () => {
      this.#timer = 0;
      this.#show(row);
      return GLib.SOURCE_REMOVE;
    });
  }

  /** @param {St.Widget} row */
  #show(row) {
    this.#label.text = row.allotHover ?? "";
    this.#label.show();
    Main.uiGroup.set_child_above_sibling(this.#label, null);
    const [x, y] = row.get_transformed_position();
    const [width, height] = row.get_transformed_size();
    const [, labelWidth] = this.#label.get_preferred_width(-1);
    const [, labelHeight] = this.#label.get_preferred_height(labelWidth);
    const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
    const below = y + height + HOVER_GAP_PX * scale;
    const monitor = Main.layoutManager.findMonitorForActor(row);
    const fits = monitor === null || below + labelHeight <= monitor.y + monitor.height;
    this.#label.set_position(
      Math.round(x + (width - labelWidth) / 2),
      Math.round(fits ? below : y - labelHeight - HOVER_GAP_PX * scale),
    );
  }

  #clearTimer() {
    if (this.#timer !== 0) {
      GLib.source_remove(this.#timer);
      this.#timer = 0;
    }
  }
}

/**
 * @param {string} swatchClass
 * @param {string} text
 * @returns {St.BoxLayout}
 */
function legendEntry(swatchClass, text) {
  const entry = new St.BoxLayout({ style_class: "allot-legend-entry" });
  entry.add_child(
    new St.Widget({
      style_class: `allot-legend-swatch ${swatchClass}`,
      y_align: Clutter.ActorAlign.CENTER,
    }),
  );
  entry.add_child(label("allot-legend-label", text));
  return entry;
}

/** @returns {St.BoxLayout} The Headroom entry, whose swatch is hatched like the bars. */
function hatchLegendEntry() {
  const entry = new St.BoxLayout({ style_class: "allot-legend-entry" });
  const swatch = new St.DrawingArea({
    style_class: "allot-legend-swatch allot-legend-hatch",
    y_align: Clutter.ActorAlign.CENTER,
  });
  swatch.connect("repaint", () => {
    const context = swatch.get_context();
    const [width, height] = swatch.get_surface_size();
    const [hasHatch, hatch] = swatch.get_theme_node().lookup_color("-allot-hatch-color", false);
    if (hasHatch) {
      paintHatch(context, hatch, 0, 0, width, height);
    }
    context.$dispose();
  });
  entry.add_child(swatch);
  entry.add_child(label("allot-legend-label", "Headroom"));
  return entry;
}

/**
 * Diagonal lines over a rectangle, which St's CSS cannot draw.
 *
 * @param {import("gi://cairo").Context} context
 * @param {import("gi://Cogl").Color} color
 * @param {number} x
 * @param {number} y
 * @param {number} width
 * @param {number} height
 */
function paintHatch(context, color, x, y, width, height) {
  const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
  const step = HATCH_STEP_PX * scale;
  context.save();
  context.rectangle(x, y, width, height);
  context.clip();
  setSource(context, color);
  context.setLineWidth(HATCH_LINE_PX * scale);
  for (let start = x - height; start < x + width; start += step) {
    context.moveTo(start, y + height);
    context.lineTo(start + height, y);
  }
  context.stroke();
  context.restore();
}

/**
 * @param {import("gi://cairo").Context} context
 * @param {number} x
 * @param {number} y
 * @param {number} width
 * @param {number} height
 * @param {number} radius
 */
function roundedRectangle(context, x, y, width, height, radius) {
  const right = x + width;
  const bottom = y + height;
  context.arc(x + radius, y + radius, radius, Math.PI, 1.5 * Math.PI);
  context.arc(right - radius, y + radius, radius, 1.5 * Math.PI, 2 * Math.PI);
  context.arc(right - radius, bottom - radius, radius, 0, Math.PI / 2);
  context.arc(x + radius, bottom - radius, radius, Math.PI / 2, Math.PI);
  context.closePath();
}
