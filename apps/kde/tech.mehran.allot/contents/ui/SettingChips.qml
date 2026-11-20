import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// A string list as chips with remove buttons, and a field that adds one (DESIGN.md section 3).
// Adding or removing saves; in an edit dialog the draft keeps it until Save.
ColumnLayout {
    id: row

    property var host: null
    property var setting: null
    property bool inDialog: false
    // Off where the group's title already names the list, as Print-only flags does.
    property bool showLabel: true
    readonly property var view: {
        row.host.revision;
        return View.chipsView(row.setting, row.host.number, !row.inDialog && row.host.readOnly,
            !row.inDialog);
    }
    // Rebuilt only when they change, so a focused chip survives the refresh.
    property var chips: []
    property string chipsSignature: ""

    function syncChips() {
        const signature = View.signature(row.view.chips);
        if (signature !== row.chipsSignature) {
            row.chipsSignature = signature;
            row.chips = row.view.chips;
        }
    }

    function add() {
        if (entry.text.trim() === "") {
            return;
        }
        if (row.setting.add(entry.text)) {
            entry.text = "";
        }
        row.host.changed();
    }

    Layout.fillWidth: true
    spacing: Kirigami.Units.smallSpacing

    onViewChanged: row.syncChips()
    Component.onCompleted: row.syncChips()

    RowLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Layout.fillWidth: true
            text: row.view.label
            visible: row.showLabel
            wrapMode: Text.Wrap
        }

        HelpLabel {
            text: row.view.help
            visible: !row.showLabel
        }

        ResetButton {
            name: row.view.resetName
            visible: row.view.resetVisible
            onClicked: row.host.resetSetting(row.setting)
        }
    }

    HelpLabel {
        text: row.view.help
        visible: row.showLabel
    }

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing
        visible: row.chips.length > 0

        Repeater {
            model: row.chips

            Kirigami.Chip {
                required property var modelData
                required property int index

                text: modelData.text
                closable: true
                checkable: false
                enabled: row.view.enabled
                Accessible.name: modelData.name
                onRemoved: {
                    row.setting.remove(index);
                    row.host.changed();
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true

        QQC2.TextField {
            id: entry

            Layout.fillWidth: true
            placeholderText: row.view.addLabel
            enabled: row.view.enabled
            Accessible.name: row.view.addLabel
            onAccepted: row.add()
        }

        QQC2.Button {
            text: "Add"
            icon.name: "list-add"
            enabled: row.view.enabled && entry.text.trim() !== ""
            onClicked: row.add()
        }
    }

    RowNotes {
        view: row.view
    }
}
