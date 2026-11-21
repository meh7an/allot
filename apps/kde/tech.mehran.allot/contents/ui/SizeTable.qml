import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// A class or preset table as rows with an aligned summary; Edit opens the size dialog (DESIGN.md
// D5, section 10 KDE). The rows rebuild only when they change, so focus survives the refresh.
SettingsSection {
    id: table

    property var host: null
    property var section: null
    property var items: []
    // The rows' summaries hold the fit counts, which change with the machine; they update in
    // place, so a fit change never rebuilds the rows and drops the focus on Edit (M7.1).
    property var summaries: []
    property string shownSignature: ""

    function sync() {
        if (table.host.model === null) {
            return;
        }
        const items = View.sizeRowItems(table.section.table, table.host.model.fits,
            table.host.number);
        const summaries = items.map((item) => item.summary);
        if (View.signature(summaries) !== View.signature(table.summaries)) {
            table.summaries = summaries;
        }
        const signature = View.signature([table.section.table.value, table.host.readOnly]);
        if (signature !== table.shownSignature) {
            table.shownSignature = signature;
            table.items = items;
        }
    }

    // Where the size dialog returns the focus: the row's Edit button, by name, because a saved row
    // is rebuilt; a row that is gone (a new preset) leaves it on Add Preset.
    function focusRow(names) {
        Qt.callLater(() => {
            for (const name of names) {
                const index = table.items.findIndex((item) => item.name === name);
                if (index >= 0) {
                    rows.itemAt(index)?.editButton.forceActiveFocus(Qt.OtherFocusReason);
                    return;
                }
            }
            addButton.forceActiveFocus(Qt.OtherFocusReason);
        });
    }

    function remove(view) {
        const removed = View.removeSizeRow(table.section.table, view);
        table.host.notice(removed.text, removed.undo);
        table.host.changed();
    }

    title: table.section.title

    Connections {
        target: table.host

        function onRevisionChanged() {
            table.sync();
        }
    }

    Component.onCompleted: table.sync()

    Repeater {
        id: rows

        model: table.items

        RowLayout {
            id: sizeRow

            required property var modelData
            required property int index
            readonly property string summary: table.summaries[index] ?? modelData.summary

            readonly property Item editButton: edit

            Layout.fillWidth: true
            enabled: !table.host.readOnly

            // One summary for a screen reader, such as "CPU heavy, 8 cores, ...": the two lines
            // are not read apart. The buttons beside it say which row they act on.
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0
                Accessible.role: Accessible.StaticText
                Accessible.name: View.sizeRowReading({
                    label: sizeRow.modelData.label,
                    summary: sizeRow.summary,
                })
                Accessible.description: modelData.keyText

                QQC2.Label {
                    Layout.fillWidth: true
                    text: modelData.label
                    wrapMode: Text.Wrap
                    Accessible.ignored: true
                }

                HelpLabel {
                    text: (modelData.keyText === "" ? ""
                        : `<tt>${View.escaped(modelData.keyText)}</tt>&nbsp;&nbsp;`)
                        + View.escaped(sizeRow.summary)
                    Accessible.ignored: true
                }
            }

            ResetButton {
                name: modelData.resetName ?? ""
                visible: modelData.resetName !== null
                onClicked: table.remove(modelData.view)
            }

            IconButton {
                iconName: "edit-delete"
                name: modelData.deleteName ?? ""
                visible: modelData.deleteName !== null
                onClicked: table.host.confirm(View.deletePresetConfirmation(modelData.name),
                    () => table.remove(modelData.view))
            }

            IconButton {
                id: edit

                iconName: "document-edit"
                name: modelData.editName
                onClicked: {
                    const draft = table.section.table.edit(modelData.name);
                    if (draft !== null) {
                        table.host.openSizeDialog(table.section.table, draft, table);
                    }
                }
            }
        }
    }

    QQC2.Button {
        id: addButton

        text: "Add Preset…"
        icon.name: "list-add"
        visible: table.section.table.canAdd
        enabled: !table.host.readOnly
        onClicked: table.host.openSizeDialog(table.section.table, table.section.table.add(), table)
    }
}
