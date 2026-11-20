import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

import "../code/settings-view.mjs" as View

// An on/off setting: a CheckBox, as KDE forms use (DESIGN.md section 3), its help and key under
// it, and its reset button.
ColumnLayout {
    id: row

    property var host: null
    property var setting: null
    readonly property var view: {
        row.host.revision;
        return View.switchView(row.setting, row.host.number, row.host.readOnly);
    }

    Layout.fillWidth: true
    spacing: 0

    RowLayout {
        Layout.fillWidth: true

        QQC2.CheckBox {
            id: box

            Layout.fillWidth: true
            text: row.view.label
            checked: row.view.checked
            enabled: row.view.enabled
            Accessible.description: row.setting.spec.help + " " + row.view.key
            onToggled: {
                row.setting.set(checked);
                row.host.changed();
            }
        }

        ResetButton {
            name: row.view.resetName
            visible: row.view.resetVisible
            onClicked: row.host.resetSetting(row.setting)
        }
    }

    HelpLabel {
        Layout.leftMargin: box.indicator.width + box.spacing
        text: row.view.help
    }

    RowNotes {
        Layout.leftMargin: box.indicator.width + box.spacing
        view: row.view
    }
}
