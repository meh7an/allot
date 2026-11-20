// Generated from apps/gnome/allot@mehran.tech/lib/setting-values.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/**
 * @import {
 *   Bound,
 *   Change,
 *   ChoiceOption,
 *   HeavyRule,
 *   JobProfile,
 *   Quantity,
 *   SettingControl,
 *   SettingSpec,
 *   SizeRow,
 *   SizeRowSpec,
 * } from "../contract.g.mjs"
 */
// The view models of the Settings window's values, one per kind of value, shared by every page
// and by the class, preset and rule dialogs. They hold no I/O: a page's host, the settings
// session, saves what they report; a dialog's draft keeps it until Save.
/**
 * What is wrong with a value the user entered, for the line under its field. `service` carries
 * allotd's own reason for a refused value.
 *
 * @typedef {(
 *   | { kind: "required" }
 *   | { kind: "not_a_number" }
 *   | { kind: "whole" }
 *   | { kind: "range", min: Bound | null, max: Bound | null }
 *   | { kind: "at_least", label: string, value: number, quantity: Quantity }
 *   | { kind: "at_most", label: string, value: number, quantity: Quantity }
 *   | { kind: "blank" }
 *   | { kind: "pattern" }
 *   | { kind: "duplicate" }
 *   | { kind: "name_taken" }
 *   | { kind: "service", message: string }
 * )} Issue
 */
/**
 * Where a value model lives: a Settings page, saved by the settings session, or the draft of
 * an edit dialog, kept until its Save.
 *
 * @typedef {object} ValueHost
 * @property {(key: string) => unknown} valueOf
 *   The value another setting or field shows now, edits included; `undefined` when none.
 * @property {(key: string) => string} labelOf
 *   The label of another setting or field, for issues that name it.
 * @property {(model: ValueModel<unknown>, changes: Change[], now: boolean) => void} edited
 *   The user changed `model`. `changes` saves it, and is empty while the model has an issue
 *   or went back to its saved value; `now` saves without waiting for the burst to end.
 */
/**
 * A unit a number can be shown in. `factor` is how many of the file's unit make one.
 *
 * @typedef {object} Unit
 * @property {string} name
 * @property {number} factor
 */
/** @type {readonly Unit[]} */
export const DURATION_UNITS = [
    { name: "seconds", factor: 1 },
    { name: "minutes", factor: 60 },
    { name: "hours", factor: 3600 },
];
// A duration turned back on with neither an earlier value nor a default.
const MINUTE_SECS = 60;
/** @type {readonly Unit[]} */
export const MEMORY_UNITS = [
    { name: "MB", factor: 1 },
    { name: "GB", factor: 1024 },
];
/**
 * One value of the Settings window, with its saved value, the value shown, and an issue when
 * the shown value cannot be saved.
 *
 * @template T
 */
export class ValueModel {
    /**
     * @param {SettingSpec} spec
     * @param {ValueHost} host
     * @param {T} value The saved value.
     */
    constructor(spec, host, value) {
        /** @type {Issue | null} */
        this.issue = null;
        /**
         * Bumped on every edit, so a save that answers late marks only the edit it carried as saved.
         */
        this.generation = 0;
        this._dirty = false;
        this.spec = spec;
        this.host = host;
        this._saved = value;
        this._value = value;
    }
    get key() {
        return this.spec.key;
    }
    get label() {
        return this.spec.label;
    }
    /** The value the field shows: the saved one, or the user's edit. */
    get value() {
        return this._value;
    }
    /** The value allotd last reported. */
    get saved() {
        return this._saved;
    }
    /** Whether the shown value is the user's edit, not yet saved. */
    get dirty() {
        return this._dirty;
    }
    /**
     * The value while the key is not in the file, or `undefined` when the setting has none.
     *
     * @returns {T | undefined}
     */
    get defaultValue() {
        return undefined;
    }
    /** Whether the row shows its reset button: the saved value differs from the default. */
    get canReset() {
        const fallback = this.defaultValue;
        return fallback !== undefined && !sameValue(this._saved, fallback);
    }
    /** Whether the switch this setting needs is on; a disabled setting keeps its value. */
    get enabled() {
        const requires = this.spec.requires;
        return requires === null || this.host.valueOf(requires.key) !== false;
    }
    /** Why the setting is disabled, or `null` while it is enabled. */
    get disabledNote() {
        var _a;
        var _b;
        return this.enabled ? null : ((_b = (_a = this.spec.requires) === null || _a === void 0 ? void 0 : _a.note) !== null && _b !== void 0 ? _b : null);
    }
    /**
     * Takes the value allotd reports. An edit not yet saved stays on screen; a hand edit to the
     * same key then meets it as a conflict when it is saved.
     *
     * @param {T} saved
     */
    load(saved) {
        this._saved = saved;
        if (!this._dirty) {
            this._value = saved;
            this.loaded();
        }
    }
    /** Removes the key from the file, so its default applies again. */
    reset() {
        const fallback = this.defaultValue;
        if (fallback === undefined) {
            return;
        }
        this.show(fallback, null);
        this.host.edited(this, [{ kind: "reset", key: this.key }], false);
    }
    /** Shows the saved value again and drops the edit and its issue. */
    undo() {
        this.issue = null;
        this._value = this._saved;
        this._dirty = false;
        this.generation += 1;
        this.loaded();
        this.host.edited(this, [], false);
    }
    /**
     * Marks the edit of `generation` as saved; a later edit stays dirty.
     *
     * @param {number} generation
     */
    markSaved(generation) {
        if (generation === this.generation && this.issue === null) {
            this._dirty = false;
        }
    }
    /**
     * Shows allotd's reason for refusing the value; the value stays for the user to fix.
     *
     * @param {string} message
     */
    refuse(message) {
        this.issue = { kind: "service", message };
    }
    /**
     * allotd's reason for a refused value as a sentence that names it, such as "Headroom must be
     * at least 0 and below 100".
     *
     * @param {string} problemKey The dotted key allotd reported, this setting's or inside it.
     * @param {string} message What the value must be.
     * @returns {string}
     */
    describeProblem(problemKey, message) {
        return `${this.label} ${message}`;
    }
    /**
     * Shows `value` as the user's edit.
     *
     * @protected
     * @param {T} value
     * @param {Issue | null} issue
     */
    show(value, issue) {
        this._value = value;
        this.issue = issue;
        this._dirty = true;
        this.generation += 1;
    }
    /**
     * Shows `value` and reports it to the host: saved with `change` when it has no issue.
     *
     * @protected
     * @param {T} value
     * @param {Issue | null} issue
     * @param {Change} change
     * @param {boolean} now
     */
    apply(value, issue, change, now) {
        this.show(value, issue);
        this.host.edited(this, issue === null ? [change] : [], now);
    }
    /**
     * Shows `issue` without changing the value.
     *
     * @protected
     * @param {Issue | null} issue
     */
    flag(issue) {
        this.issue = issue;
    }
    /**
     * Called after the shown value was replaced by the saved one.
     *
     * @protected
     */
    loaded() { }
}
/**
 * An on/off setting.
 *
 * @extends {ValueModel<boolean>}
 */
export class SwitchModel extends ValueModel {
    /**
     * @param {SettingSpec} spec
     * @param {ValueHost} host
     * @param {boolean} value
     */
    constructor(spec, host, value) {
        super(spec, host, value);
    }
    /** @returns {boolean | undefined} */
    get defaultValue() {
        return this.spec.control.kind === "switch" ? this.spec.control.default : undefined;
    }
    /** @param {boolean} on */
    set(on) {
        this.apply(on, null, { kind: "set_switch", key: this.key, value: on }, false);
    }
}
/**
 * A number in the unit of its setting: percent, runs, days, cores or a port. Durations and
 * memory sizes add a unit the user picks. `null` is a value only where the setting can be
 * automatic.
 *
 * @extends {ValueModel<number | null>}
 */
export class NumberModel extends ValueModel {
    /**
     * @param {SettingSpec} spec
     * @param {ValueHost} host
     * @param {number | null} value
     * @param {readonly Unit[]} [units] The units the value can be shown in, smallest first.
     */
    constructor(spec, host, value, units = []) {
        super(spec, host, value);
        this._units = units;
        this.unit = bestUnit(units, value);
    }
    /** @returns {Extract<SettingControl, { kind: "number" }>} */
    get control() {
        const control = this.spec.control;
        if (control.kind !== "number") {
            throw new Error(`${this.key} is not a number`);
        }
        return control;
    }
    get quantity() {
        return this.control.quantity;
    }
    /** @returns {number | null | undefined} */
    get defaultValue() {
        const control = this.control;
        if (control.default !== null) {
            return control.default;
        }
        return control.automatic !== null ? null : undefined;
    }
    /** The units the user can pick from; empty when the number has only its own. */
    get units() {
        return this._units;
    }
    /** The value in the picked unit, or `null` while automatic. */
    get shown() {
        const value = this.value;
        return value === null ? null : value / this.unit.factor;
    }
    /** Whether the value is worked out by allotd, as `automatic` describes. */
    get automatic() {
        return this.value === null;
    }
    /**
     * Shows the value in another unit; the value itself does not change.
     *
     * @param {string} name
     */
    setUnit(name) {
        const unit = this._units.find((candidate) => candidate.name === name);
        if (unit !== undefined) {
            this.unit = unit;
        }
    }
    /**
     * Commits a number the user entered in the picked unit: on Enter, on leaving the field, or
     * on a spin step.
     *
     * @param {number} shown
     */
    commit(shown) {
        const value = this._units.length > 0 ? round(shown * this.unit.factor) : shown;
        this.apply(value, this.check(value), { kind: "set_number", key: this.key, value }, false);
    }
    /**
     * Leaves the value to allotd, or gives it the number shown before.
     *
     * @param {boolean} automatic
     * @param {number} [value] The number to give it when `automatic` is false.
     */
    setAutomatic(automatic, value) {
        if (automatic) {
            this.apply(null, null, { kind: "reset", key: this.key }, false);
        }
        else if (value !== undefined) {
            this.commit(value / this.unit.factor);
        }
    }
    /**
     * Checks the shown value again, after a value it is compared with changed.
     */
    recheck() {
        var _a;
        const value = this.value;
        if (value !== null && ((_a = this.issue) === null || _a === void 0 ? void 0 : _a.kind) !== "service") {
            this.flag(this.check(value));
        }
    }
    /**
     * What is wrong with `value` for this setting, or `null`.
     *
     * @param {number} value
     * @returns {Issue | null}
     */
    check(value) {
        const control = this.control;
        if (!Number.isFinite(value)) {
            return { kind: "not_a_number" };
        }
        if (control.integer && !Number.isInteger(value)) {
            return { kind: "whole" };
        }
        if (outside(value, control.min, control.max)) {
            return { kind: "range", min: control.min, max: control.max };
        }
        if (control.min_key !== null) {
            const least = this._other(control.min_key);
            if (least !== null && value < least) {
                const label = this.host.labelOf(control.min_key);
                return { kind: "at_least", label, value: least, quantity: this.quantity };
            }
        }
        if (control.max_key !== null) {
            const most = this._other(control.max_key);
            if (most !== null && value > most) {
                const label = this.host.labelOf(control.max_key);
                return { kind: "at_most", label, value: most, quantity: this.quantity };
            }
        }
        return null;
    }
    /**
     * @protected
     */
    loaded() {
        this.unit = bestUnit(this._units, this.value);
    }
    /**
     * @param {string} key
     * @returns {number | null}
     */
    _other(key) {
        const value = this.host.valueOf(key);
        return typeof value === "number" ? value : null;
    }
}
/**
 * A duration in seconds, shown in seconds, minutes or hours. Where one value turns the feature
 * off (`off` in the spec), a switch in front of it writes that value.
 */
export class DurationModel extends NumberModel {
    /**
     * @param {SettingSpec} spec
     * @param {ValueHost} host
     * @param {number | null} value
     */
    constructor(spec, host, value) {
        super(spec, host, value, DURATION_UNITS);
        /** @type {number | null} */
        this._lastOn = null;
    }
    /** The switch in front of the duration, or `null` when it has none. */
    get off() {
        return this.control.off;
    }
    /** Whether the feature is on; always true without an off switch. */
    get isOn() {
        const off = this.control.off;
        return off === null || this.value !== off.value;
    }
    /**
     * Turns the feature off by writing the off value, or on again with the duration it had.
     *
     * @param {boolean} on
     */
    setOn(on) {
        var _a, _b;
        const off = this.control.off;
        if (off === null || on === this.isOn) {
            return;
        }
        if (!on) {
            this._lastOn = this.value;
            this.apply(off.value, null, { kind: "set_number", key: this.key, value: off.value }, false);
            return;
        }
        const value = (_b = (_a = this._lastOn) !== null && _a !== void 0 ? _a : this.control.default) !== null && _b !== void 0 ? _b : MINUTE_SECS;
        this.commit(value / this.unit.factor);
    }
    /** @protected */
    loaded() {
        super.loaded();
        if (this.off !== null && this.value === this.off.value) {
            this.unit = bestUnit(DURATION_UNITS, this.control.default);
        }
    }
}
/** A size in megabytes, shown in MB or GB; the file always gets MB. */
export class MemoryModel extends NumberModel {
    /**
     * @param {SettingSpec} spec
     * @param {ValueHost} host
     * @param {number | null} value
     */
    constructor(spec, host, value) {
        super(spec, host, value, MEMORY_UNITS);
    }
}
/**
 * A list of strings shown as chips, such as the print-only flags; saved on add or remove.
 *
 * @extends {ValueModel<string[]>}
 */
export class StringListModel extends ValueModel {
    /**
     * @param {SettingSpec} spec
     * @param {ValueHost} host
     * @param {string[]} value
     */
    constructor(spec, host, value) {
        super(spec, host, value);
    }
    /** @returns {string[] | undefined} */
    get defaultValue() {
        return this.spec.control.kind === "string_list" ? this.spec.control.default : undefined;
    }
    /**
     * Adds an entry; refuses a blank one or one the list already has.
     *
     * @param {string} text
     * @returns {boolean} Whether it was added.
     */
    add(text) {
        const entry = text.trim();
        if (entry === "") {
            this.flag({ kind: "blank" });
            return false;
        }
        if (this.value.includes(entry)) {
            this.flag({ kind: "duplicate" });
            return false;
        }
        this._save([...this.value, entry]);
        return true;
    }
    /** @param {number} index */
    remove(index) {
        if (index >= 0 && index < this.value.length) {
            this._save(this.value.filter((_, at) => at !== index));
        }
    }
    /** @param {string[]} entries */
    _save(entries) {
        this.apply(entries, null, { kind: "set_list", key: this.key, value: entries }, true);
    }
}
/**
 * One row of a size table as a page lists it.
 *
 * @typedef {object} SizeRowView
 * @property {SizeRow} row
 * @property {string} label
 * @property {boolean} builtIn
 *   Built-in rows can be edited and reset, never renamed or deleted.
 * @property {boolean} changed
 *   A built-in row whose values differ from its defaults; always true for a row the user added.
 */
/**
 * A size table, classes or presets: rows edited one at a time in a dialog, each Save written at
 * once.
 *
 * @extends {ValueModel<SizeRow[]>}
 */
export class SizeTableModel extends ValueModel {
    /**
     * @param {SettingSpec} spec
     * @param {ValueHost} host
     * @param {SizeRow[]} value
     */
    constructor(spec, host, value) {
        super(spec, host, value);
    }
    /** @returns {Extract<SettingControl, { kind: "size_table" }>} */
    get control() {
        const control = this.spec.control;
        if (control.kind !== "size_table") {
            throw new Error(`${this.key} is not a size table`);
        }
        return control;
    }
    /** @returns {SizeRow[]} */
    get defaultValue() {
        return this.control.rows.map(builtInRow);
    }
    get canReset() {
        return this.rows.some((row) => row.changed);
    }
    /** Whether the user can add, rename and delete rows. */
    get canAdd() {
        return this.control.new_row_name !== null;
    }
    /** @returns {SizeRowView[]} */
    get rows() {
        return this.value.map((row) => {
            var _a;
            const spec = this._builtIn(row.name);
            return {
                row,
                label: (_a = spec === null || spec === void 0 ? void 0 : spec.label) !== null && _a !== void 0 ? _a : row.name,
                builtIn: spec !== undefined,
                changed: spec === undefined || !sameValue(row, builtInRow(spec)),
            };
        });
    }
    /**
     * A draft of the row `name` for its edit dialog.
     *
     * @param {string} name
     * @returns {RowDraft | null} `null` when the table has no such row.
     */
    edit(name) {
        const row = this.value.find((candidate) => candidate.name === name);
        return row === undefined ? null : new RowDraft(this, row, this._builtIn(name) !== undefined);
    }
    /** A draft of a new row, with the defaults a new row starts with. */
    add() {
        return new RowDraft(this, null, false);
    }
    /**
     * Writes the draft's row, renaming it when its name changed. Nothing is written while the
     * draft has an issue.
     *
     * @param {RowDraft} draft
     * @returns {boolean} Whether it was handed over to be written.
     */
    save(draft) {
        if (!draft.check()) {
            return false;
        }
        const row = draft.row();
        /** @type {Change[]} */
        const changes = [];
        if (draft.original !== null && draft.original !== row.name) {
            changes.push({ kind: "reset_row", table: this.key, name: draft.original });
        }
        changes.push({
            kind: "set_row",
            table: this.key,
            name: row.name,
            profile: row.profile,
            min_cores: row.min_cores,
        });
        const rows = this.value.filter((candidate) => candidate.name !== draft.original && candidate.name !== row.name);
        this._write(sortRows(this.key, [...rows, row], this.control.rows), changes);
        return true;
    }
    /**
     * Deletes a row the user added; a built-in row is reset instead.
     *
     * @param {string} name
     */
    remove(name) {
        const spec = this._builtIn(name);
        const rows = spec === undefined
            ? this.value.filter((row) => row.name !== name)
            : this.value.map((row) => (row.name === name ? builtInRow(spec) : row));
        this._write(rows, [{ kind: "reset_row", table: this.key, name }]);
    }
    /**
     * Names the row and field of `presets.<name>.<field>`, or the name itself for
     * `presets.<name>`.
     *
     * @param {string} problemKey
     * @param {string} message
     * @returns {string}
     */
    describeProblem(problemKey, message) {
        var _a, _b;
        var _c, _d, _e;
        const inside = problemKey.slice(this.key.length + 1);
        const dot = inside.indexOf(".");
        if (inside === "") {
            return super.describeProblem(problemKey, message);
        }
        const name = dot < 0 ? inside : inside.slice(0, dot);
        const fieldKey = dot < 0 ? null : inside.slice(dot + 1);
        const field = this.control.fields.find((candidate) => candidate.key === fieldKey);
        const label = (_d = (_c = field === null || field === void 0 ? void 0 : field.label) !== null && _c !== void 0 ? _c : (_a = this.control.new_row_name) === null || _a === void 0 ? void 0 : _a.label) !== null && _d !== void 0 ? _d : "Name";
        return `${(_e = (_b = this._builtIn(name)) === null || _b === void 0 ? void 0 : _b.label) !== null && _e !== void 0 ? _e : name}: ${label} ${message}`;
    }
    /**
     * The names a new or renamed row cannot take: every other row's, and every class's.
     *
     * @param {string | null} original The row's own name, which it may keep.
     * @returns {string[]}
     */
    takenNames(original) {
        const names = this.value.map((row) => row.name).filter((name) => name !== original);
        const classes = this.key === "classes" ? [] : this.host.valueOf("classes");
        if (Array.isArray(classes)) {
            for (const row of classes) {
                names.push(/** @type {SizeRow} */ (row).name);
            }
        }
        return names;
    }
    /**
     * @param {SizeRow[]} rows
     * @param {Change[]} changes
     */
    _write(rows, changes) {
        this.show(rows, null);
        this.host.edited(this, changes, true);
    }
    /** @param {string} name */
    _builtIn(name) {
        return this.control.rows.find((row) => row.name === name);
    }
}
/** The fields of a class or preset in its edit dialog, kept until Save. */
export class RowDraft {
    /**
     * @param {SizeTableModel} table
     * @param {SizeRow | null} row
     * @param {boolean} builtIn
     */
    constructor(table, row, builtIn) {
        var _a, _b;
        /** @type {Issue | null} */
        this.nameIssue = null;
        /** @type {(() => void) | null} */
        this.onChange = null;
        this._table = table;
        this.original = (_a = row === null || row === void 0 ? void 0 : row.name) !== null && _a !== void 0 ? _a : null;
        this.name = (_b = row === null || row === void 0 ? void 0 : row.name) !== null && _b !== void 0 ? _b : "";
        this.builtIn = builtIn;
        /** @type {ValueHost} */
        const host = {
            valueOf: (key) => { var _a; return (_a = this.field(key)) === null || _a === void 0 ? void 0 : _a.value; },
            labelOf: (key) => { var _a; var _b; return (_b = (_a = this.field(key)) === null || _a === void 0 ? void 0 : _a.label) !== null && _b !== void 0 ? _b : key; },
            edited: () => this._edited(),
        };
        this.fields = table.control.fields.map((spec) => {
            const value = row === null ? newRowValue(spec) : rowValue(row, spec.key);
            return /** @type {NumberModel} */ (valueModel(spec, host, value));
        });
    }
    /** Whether the name can be edited: rows the user added, in a table that allows them. */
    get canRename() {
        return this._table.canAdd && !this.builtIn;
    }
    /** The name field's label, help and pattern, when the table allows names. */
    get nameSpec() {
        return this._table.control.new_row_name;
    }
    /**
     * @param {string} key
     * @returns {NumberModel | undefined}
     */
    field(key) {
        return this.fields.find((field) => field.key === key);
    }
    /** @param {string} text */
    setName(text) {
        var _a;
        this.name = text.trim();
        this.nameIssue = this._nameIssue();
        (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this);
    }
    /**
     * Checks every field and the name, showing each issue.
     *
     * @returns {boolean} Whether the row can be saved.
     */
    check() {
        var _a;
        this.nameIssue = this._nameIssue();
        for (const field of this.fields) {
            if (field.value === null) {
                field.issue = field.control.automatic === null ? { kind: "required" } : null;
            }
            else {
                field.recheck();
            }
        }
        (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this);
        return this.nameIssue === null && this.fields.every((field) => field.issue === null);
    }
    /** @returns {SizeRow} */
    row() {
        var _a;
        var _b;
        /** @param {string} key */
        const number = (key) => { var _a; var _b; return (_b = (_a = this.field(key)) === null || _a === void 0 ? void 0 : _a.value) !== null && _b !== void 0 ? _b : 0; };
        return {
            name: this.name,
            profile: {
                cores: number("cores"),
                ram_mb: number("ram_mb"),
                gpu_pct: number("gpu_pct"),
                vram_mb: number("vram_mb"),
                est_secs: number("est_secs"),
            },
            min_cores: (_b = (_a = this.field("min_cores")) === null || _a === void 0 ? void 0 : _a.value) !== null && _b !== void 0 ? _b : null,
        };
    }
    /** @returns {Issue | null} */
    _nameIssue() {
        const spec = this.nameSpec;
        if (spec === null || !this.canRename) {
            return null;
        }
        if (this.name === "") {
            return { kind: "blank" };
        }
        if (!new RegExp(spec.pattern).test(this.name)) {
            return { kind: "pattern" };
        }
        return this._table.takenNames(this.original).includes(this.name)
            ? { kind: "name_taken" }
            : null;
    }
    _edited() {
        var _a;
        for (const field of this.fields) {
            field.recheck();
        }
        (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this);
    }
}
/**
 * The heavy-command rules: an ordered list edited one rule at a time in a dialog, reordered and
 * deleted from each row's menu, and written whole on every change.
 *
 * @extends {ValueModel<HeavyRule[]>}
 */
export class RuleListModel extends ValueModel {
    /**
     * @param {SettingSpec} spec
     * @param {ValueHost} host
     * @param {HeavyRule[]} value
     */
    constructor(spec, host, value) {
        super(spec, host, value);
    }
    /** @returns {Extract<SettingControl, { kind: "rule_list" }>} */
    get control() {
        const control = this.spec.control;
        if (control.kind !== "rule_list") {
            throw new Error(`${this.key} is not a rule list`);
        }
        return control;
    }
    /** @returns {HeavyRule[]} */
    get defaultValue() {
        return this.control.default;
    }
    /**
     * Names the rule, from 1, and its field for `classifier.rules[<i>].<field>`.
     *
     * @param {string} problemKey
     * @param {string} message
     * @returns {string}
     */
    describeProblem(problemKey, message) {
        const match = /^\[(\d+)\](?:\.(\w+))?$/.exec(problemKey.slice(this.key.length));
        if (match === null) {
            return super.describeProblem(problemKey, message);
        }
        const field = this.control.fields.find((candidate) => candidate.key === match[2]);
        const subject = field === undefined ? "" : ` ${field.label}`;
        return `Rule ${Number(match[1]) + 1}:${subject} ${message}`;
    }
    /**
     * A draft of the rule at `index` for its edit dialog.
     *
     * @param {number} index
     * @returns {RuleDraft | null}
     */
    edit(index) {
        const rule = this.value[index];
        return rule === undefined ? null : new RuleDraft(this.control.fields, rule, index);
    }
    /** A draft of a new rule, added at the end. */
    add() {
        return new RuleDraft(this.control.fields, null, null);
    }
    /**
     * Writes the list with the draft's rule in its place.
     *
     * @param {RuleDraft} draft
     * @returns {boolean} Whether it was handed over to be written.
     */
    save(draft) {
        if (!draft.check()) {
            return false;
        }
        const rules = [...this.value];
        if (draft.index === null) {
            rules.push(draft.rule());
        }
        else {
            rules[draft.index] = draft.rule();
        }
        this._write(rules);
        return true;
    }
    /**
     * Moves a rule; rules match in list order.
     *
     * @param {number} from
     * @param {number} to
     */
    move(from, to) {
        const rules = [...this.value];
        const [rule] = rules.splice(from, 1);
        if (rule === undefined || to < 0 || to > rules.length) {
            return;
        }
        rules.splice(to, 0, rule);
        this._write(rules);
    }
    /** @param {number} index */
    remove(index) {
        if (index >= 0 && index < this.value.length) {
            this._write(this.value.filter((_, at) => at !== index));
        }
    }
    /** @param {HeavyRule[]} rules */
    _write(rules) {
        this.apply(rules, null, { kind: "set_rules", rules }, true);
    }
}
/** The fields of one heavy-command rule in its edit dialog, kept until Save. */
export class RuleDraft {
    /**
     * @param {SettingSpec[]} fields The rule's fields from the spec.
     * @param {HeavyRule | null} rule
     * @param {number | null} index
     */
    constructor(fields, rule, index) {
        var _a;
        var _b, _c, _d, _e;
        /** @type {Issue | null} */
        this.programIssue = null;
        /** @type {Issue | null} */
        this.classIssue = null;
        /** @type {(() => void) | null} */
        this.onChange = null;
        this._fields = fields;
        this.index = index;
        this.program = (_b = rule === null || rule === void 0 ? void 0 : rule.program) !== null && _b !== void 0 ? _b : "";
        this.ruleClass = (_d = (_c = rule === null || rule === void 0 ? void 0 : rule.class) !== null && _c !== void 0 ? _c : (_a = this._choice()) === null || _a === void 0 ? void 0 : _a.default) !== null && _d !== void 0 ? _d : null;
        /** @type {ValueHost} */
        const host = {
            valueOf: () => undefined,
            labelOf: (key) => key,
            edited: () => { var _a; return (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this); },
        };
        this.args = new StringListModel(this._field("args"), host, (_e = rule === null || rule === void 0 ? void 0 : rule.args) !== null && _e !== void 0 ? _e : []);
    }
    /** The program field's spec. */
    get programSpec() {
        return this._field("program");
    }
    /** The class field's spec. */
    get classSpec() {
        return this._field("class");
    }
    /** @returns {ChoiceOption[]} */
    get classOptions() {
        var _a;
        var _b;
        return (_b = (_a = this._choice()) === null || _a === void 0 ? void 0 : _a.options) !== null && _b !== void 0 ? _b : [];
    }
    /** @param {string} text */
    setProgram(text) {
        var _a;
        this.program = text.trim();
        this.programIssue = this._programIssue();
        (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this);
    }
    /** @param {string} value */
    setClass(value) {
        var _a;
        this.ruleClass = value;
        this.classIssue = null;
        (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this);
    }
    /** @returns {boolean} Whether the rule can be saved. */
    check() {
        var _a;
        this.programIssue = this._programIssue();
        this.classIssue = this.ruleClass === null ? { kind: "required" } : null;
        (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this);
        return this.programIssue === null && this.classIssue === null;
    }
    /** @returns {HeavyRule} */
    rule() {
        var _a;
        return {
            program: this.program,
            args: this.args.value,
            class: /** @type {HeavyRule["class"]} */ ((_a = this.ruleClass) !== null && _a !== void 0 ? _a : ""),
        };
    }
    /** @returns {Issue | null} */
    _programIssue() {
        if (this.program === "") {
            return { kind: "blank" };
        }
        const control = this.programSpec.control;
        const pattern = control.kind === "text" ? control.pattern : null;
        return pattern !== null && !new RegExp(pattern).test(this.program)
            ? { kind: "pattern" }
            : null;
    }
    _choice() {
        const control = this._field("class").control;
        return control.kind === "choice" ? control : null;
    }
    /** @param {string} key */
    _field(key) {
        const field = this._fields.find((candidate) => candidate.key === key);
        if (field === undefined) {
            throw new Error(`a rule has no field ${key}`);
        }
        return field;
    }
}
/**
 * The view model of a setting, by its control and quantity.
 *
 * @param {SettingSpec} spec
 * @param {ValueHost} host
 * @param {unknown} value The saved value, in the shape the control edits.
 * @returns {ValueModel<unknown>}
 */
export function valueModel(spec, host, value) {
    const control = spec.control;
    switch (control.kind) {
        case "switch":
            return new SwitchModel(spec, host, /** @type {boolean} */ (value));
        case "number": {
            const number = /** @type {number | null} */ (value);
            switch (control.quantity) {
                case "seconds":
                    return new DurationModel(spec, host, number);
                case "megabytes":
                    return new MemoryModel(spec, host, number);
                default:
                    return new NumberModel(spec, host, number);
            }
        }
        case "string_list":
            return new StringListModel(spec, host, /** @type {string[]} */ (value));
        case "size_table":
            return new SizeTableModel(spec, host, /** @type {SizeRow[]} */ (value));
        case "rule_list":
            return new RuleListModel(spec, host, /** @type {HeavyRule[]} */ (value));
        case "text":
        case "choice":
            throw new Error(`${spec.key}: ${control.kind} values live only in a rule's dialog`);
    }
}
/**
 * The sentence under a field for `issue`.
 *
 * @param {Issue} issue
 * @param {(value: number) => string} number Formats a number for the user's locale.
 * @returns {string}
 */
export function issueText(issue, number) {
    switch (issue.kind) {
        case "required":
            return "Enter a value.";
        case "not_a_number":
            return "Enter a number.";
        case "whole":
            return "Enter a whole number.";
        case "range":
            return `Enter a number ${rangeText(issue.min, issue.max, number)}.`;
        case "at_least":
            return `Must be at least ${issue.label} (${quantityText(issue.value, issue.quantity, number)}).`;
        case "at_most":
            return `Must be at most ${issue.label} (${quantityText(issue.value, issue.quantity, number)}).`;
        case "blank":
            return "Enter some text.";
        case "pattern":
            return "Use only the characters the help names.";
        case "duplicate":
            return "The list already has this.";
        case "name_taken":
            return "Another preset or a class already has this name.";
        case "service":
            return issue.message;
    }
}
/**
 * A value with its unit, such as "15 %", "3 minutes" or "2 GB". Durations take the largest unit
 * that divides them, and memory GB when it is a whole number of them.
 *
 * @param {number} value In the setting's own unit.
 * @param {Quantity} quantity
 * @param {(value: number) => string} number Formats a number for the user's locale.
 * @returns {string}
 */
export function quantityText(value, quantity, number) {
    switch (quantity) {
        case "percent":
            return `${number(value)} %`;
        case "runs":
            return plural(value, "run", number);
        case "days":
            return plural(value, "day", number);
        case "cores":
            return plural(value, "core", number);
        case "seconds": {
            const unit = bestUnit(DURATION_UNITS, value);
            return plural(value / unit.factor, unit.name.slice(0, -1), number);
        }
        case "megabytes": {
            const unit = bestUnit(MEMORY_UNITS, value);
            return `${number(value / unit.factor)} ${unit.name}`;
        }
        case "port":
            return String(value);
    }
}
/**
 * Deep equality of plain JSON values, ignoring the order of object keys.
 *
 * @param {unknown} a
 * @param {unknown} b
 * @returns {boolean}
 */
export function sameValue(a, b) {
    if (a === b) {
        return true;
    }
    if (a === null || b === null || typeof a !== "object" || typeof b !== "object") {
        return false;
    }
    if (Array.isArray(a) || Array.isArray(b)) {
        return (Array.isArray(a) &&
            Array.isArray(b) &&
            a.length === b.length &&
            a.every((item, index) => sameValue(item, b[index])));
    }
    const left = /** @type {Record<string, unknown>} */ (a);
    const right = /** @type {Record<string, unknown>} */ (b);
    const keys = Object.keys(left).filter((key) => left[key] !== undefined);
    return (keys.length === Object.keys(right).filter((key) => right[key] !== undefined).length &&
        keys.every((key) => sameValue(left[key], right[key])));
}
/**
 * The largest unit that shows `value` as a whole number; the smallest for 0 or automatic.
 *
 * @param {readonly Unit[]} units
 * @param {number | null} value
 * @returns {Unit}
 */
function bestUnit(units, value) {
    var _a;
    let best = (_a = units[0]) !== null && _a !== void 0 ? _a : { name: "", factor: 1 };
    if (value === null || value === 0) {
        return best;
    }
    for (const unit of units) {
        if (Number.isInteger(value / unit.factor)) {
            best = unit;
        }
    }
    return best;
}
/**
 * @param {number} value
 * @param {Bound | null} min
 * @param {Bound | null} max
 */
function outside(value, min, max) {
    const low = min !== null && (min.exclusive ? value <= min.value : value < min.value);
    const high = max !== null && (max.exclusive ? value >= max.value : value > max.value);
    return low || high;
}
/**
 * "from 0 to 99", "at least 1", "above 0", "at least 0 and below 100".
 *
 * @param {Bound | null} min
 * @param {Bound | null} max
 * @param {(value: number) => string} number
 */
function rangeText(min, max, number) {
    if (min !== null && max !== null && !min.exclusive && !max.exclusive) {
        return `from ${number(min.value)} to ${number(max.value)}`;
    }
    const low = min === null ? null : `${min.exclusive ? "above" : "at least"} ${number(min.value)}`;
    const high = max === null ? null : `${max.exclusive ? "below" : "at most"} ${number(max.value)}`;
    return [low, high].filter((part) => part !== null).join(" and ");
}
/**
 * @param {number} value
 * @param {string} noun
 * @param {(value: number) => string} number
 */
function plural(value, noun, number) {
    return `${number(value)} ${value === 1 ? noun : `${noun}s`}`;
}
// The file holds whole megabytes and seconds; a value entered in a larger unit is rounded to
// them.
/** @param {number} value */
function round(value) {
    return Math.round(value);
}
/**
 * @param {SizeRowSpec} spec
 * @returns {SizeRow}
 */
function builtInRow(spec) {
    return { name: spec.name, profile: spec.default, min_cores: spec.default_min_cores };
}
/**
 * Classes keep the spec's order; presets go by name, as allotd lists them.
 *
 * @param {string} table
 * @param {SizeRow[]} rows
 * @param {SizeRowSpec[]} builtIns
 * @returns {SizeRow[]}
 */
function sortRows(table, rows, builtIns) {
    if (table === "classes") {
        const order = builtIns.map((row) => row.name);
        return rows.sort((a, b) => order.indexOf(a.name) - order.indexOf(b.name));
    }
    return rows.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
}
/**
 * @param {SizeRow} row
 * @param {string} key
 * @returns {number | null}
 */
function rowValue(row, key) {
    var _a;
    if (key === "min_cores") {
        return row.min_cores;
    }
    /** @type {JobProfile} */
    const profile = row.profile;
    return /** @type {Record<string, number>} */ (_a = ( /** @type {unknown} */(profile))[key]) !== null && _a !== void 0 ? _a : null;
}
/**
 * @param {SettingSpec} spec
 * @returns {number | null}
 */
function newRowValue(spec) {
    return spec.control.kind === "number" ? spec.control.default : null;
}
