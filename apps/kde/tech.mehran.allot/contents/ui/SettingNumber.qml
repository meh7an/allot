import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// A number setting: its label, the number with its unit, its reset button, then its help, key,
// notes and issue. In an edit dialog it has no reset and is never read-only.
ColumnLayout {
    id: row

    property var host: null
    property var setting: null
    property bool inDialog: false
    readonly property var view: {
        row.host.revision;
        const readOnly = !row.inDialog && row.host.readOnly;
        return View.numberView(row.setting, row.host.number, readOnly,
            row.inDialog || row.host.model === null ? null : row.host.model.note(row.setting.key),
            !row.inDialog);
    }

    Layout.fillWidth: true
    Layout.leftMargin: row.view.indented ? Kirigami.Units.gridUnit * 2 : 0
    spacing: 0
    enabled: row.view.enabled

    RowLayout {
        Layout.fillWidth: true

        // The spin box carries this label as its name, so a screen reader skips this one.
        QQC2.Label {
            Layout.fillWidth: true
            text: row.view.label
            wrapMode: Text.Wrap
            Accessible.ignored: true
        }

        NumberEditor {
            host: row.host
            setting: row.setting
            view: row.view
        }

        ResetButton {
            name: row.view.resetName
            visible: row.view.resetVisible
            onClicked: row.host.resetSetting(row.setting)
        }
    }

    HelpLabel {
        text: row.view.help
    }

    RowNotes {
        view: row.view
    }
}
