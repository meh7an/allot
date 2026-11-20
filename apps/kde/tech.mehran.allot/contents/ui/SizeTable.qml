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
    property string shownSignature: ""

    function sync() {
        if (table.host.model === null) {
            return;
        }
        const items = View.sizeRowItems(table.section.table, table.host.model.fits,
            table.host.number);
        const signature = View.signature([table.section.table.value,
            items.map((item) => item.summary), table.host.readOnly]);
        if (signature !== table.shownSignature) {
            table.shownSignature = signature;
            table.items = items;
        }
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
        model: table.items

        RowLayout {
            required property var modelData

            Layout.fillWidth: true
            enabled: !table.host.readOnly

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                QQC2.Label {
                    Layout.fillWidth: true
                    text: modelData.label
                    wrapMode: Text.Wrap
                }

                HelpLabel {
                    text: (modelData.keyText === "" ? ""
                        : `<tt>${View.escaped(modelData.keyText)}</tt>&nbsp;&nbsp;`)
                        + View.escaped(modelData.summary)
                }
            }

            ResetButton {
                name: modelData.resetName ?? ""
                visible: modelData.resetName !== null
                onClicked: table.remove(modelData.view)
            }

            QQC2.ToolButton {
                icon.name: "edit-delete"
                display: QQC2.AbstractButton.IconOnly
                text: modelData.deleteName ?? ""
                visible: modelData.deleteName !== null
                Accessible.name: text
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                onClicked: table.host.confirm(View.deletePresetConfirmation(modelData.name),
                    () => table.remove(modelData.view))
            }

            QQC2.ToolButton {
                icon.name: "document-edit"
                display: QQC2.AbstractButton.IconOnly
                text: modelData.editName
                Accessible.name: text
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                onClicked: {
                    const draft = table.section.table.edit(modelData.name);
                    if (draft !== null) {
                        table.host.openSizeDialog(table.section.table, draft);
                    }
                }
            }
        }
    }

    QQC2.Button {
        text: "Add Preset…"
        icon.name: "list-add"
        visible: table.section.table.canAdd
        enabled: !table.host.readOnly
        onClicked: table.host.openSizeDialog(table.section.table, table.section.table.add())
    }
}
