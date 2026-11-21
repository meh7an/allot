import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

import "../code/settings-view.mjs" as View

// A duration that one value turns off, such as End leases left idle: the switch, then the
// duration indented under it (DESIGN.md section 3).
ColumnLayout {
    id: row

    property var host: null
    property var setting: null
    readonly property var view: {
        row.host.revision;
        return View.offSwitchView(row.setting, row.host.readOnly);
    }

    Layout.fillWidth: true
    spacing: 0

    QQC2.CheckBox {
        id: box

        Layout.fillWidth: true
        text: row.view.label
        checked: row.view.checked
        enabled: row.view.enabled
        Accessible.name: row.view.label
        Accessible.description: row.view.description
        onToggled: {
            row.setting.setOn(checked);
            row.host.changed();
        }
    }

    HelpLabel {
        Layout.leftMargin: box.indicator.width + box.spacing
        text: row.view.help
    }

    SettingNumber {
        Layout.topMargin: box.spacing
        host: row.host
        setting: row.setting
    }
}
