import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

// A number field and, for a duration or a memory size, its unit. It commits on Enter, on
// leaving the field or on a step; Escape shows the saved value again. Edits are read from
// valueChanged, guarded while the model's value is shown, because SpinBox emits no valueModified
// for a value set through AT-SPI (M0.3, DESIGN.md R11). The range is wide, so a value out of
// the setting's range shows its issue line instead of being clamped.
RowLayout {
    id: editor

    property var host: null
    property var setting: null
    property var view: null
    property bool syncing: false
    // SpinBox holds integers, so a number with decimals is held in hundredths.
    readonly property int scale: editor.view === null || editor.view.integer ? 1 : 100
    readonly property var unitNames: editor.setting === null ? []
        : editor.setting.units.map((unit) => unit.name)

    function sync() {
        if (editor.view === null) {
            return;
        }
        editor.syncing = true;
        const wanted = Math.round(editor.view.shown * editor.scale);
        if (spin.value !== wanted) {
            spin.value = wanted;
        }
        if (units.visible && units.currentIndex !== editor.view.unitIndex) {
            units.currentIndex = editor.view.unitIndex;
        }
        editor.syncing = false;
    }

    onViewChanged: editor.sync()
    Component.onCompleted: editor.sync()

    QQC2.SpinBox {
        id: spin

        from: -1000000000
        to: 1000000000
        stepSize: editor.scale
        editable: true
        Accessible.name: editor.view === null ? "" : editor.view.label
        textFromValue: (value, locale) => {
            if (editor.view === null) {
                return "";
            }
            const shown = value / editor.scale;
            // DESIGN.md section 3: a port is never grouped.
            return editor.view.grouped
                ? Number(shown).toLocaleString(locale, "f", editor.view.integer ? 0 : 2)
                : String(Math.round(shown));
        }
        valueFromText: (text, locale) => {
            const parsed = editor.view !== null && editor.view.grouped
                ? Number.fromLocaleString(locale, text) : Number(text);
            return isNaN(parsed) ? spin.value : Math.round(parsed * editor.scale);
        }
        validator: DoubleValidator {
            locale: spin.locale.name
            notation: DoubleValidator.StandardNotation
        }
        onValueChanged: {
            if (!editor.syncing && editor.setting !== null) {
                editor.setting.commit(spin.value / editor.scale);
                editor.host.changed();
            }
        }
        Keys.onEscapePressed: (event) => {
            if (editor.setting !== null && editor.setting.dirty) {
                editor.setting.undo();
                editor.host.changed();
                event.accepted = true;
            } else {
                event.accepted = false;
            }
        }
    }

    QQC2.ComboBox {
        id: units

        visible: editor.unitNames.length > 0
        model: editor.unitNames
        Accessible.name: editor.view === null ? "" : `${editor.view.label} unit`
        onActivated: (index) => {
            const name = editor.unitNames[index];
            if (name !== undefined) {
                editor.setting.setUnit(name);
                editor.host.changed();
            }
        }
    }
}
