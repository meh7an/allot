import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

// The Service page's status row: running, stopped by you, not answering or starting, with its
// button, and Open Logs while something is wrong.
SettingsSection {
    id: service

    property var host: null
    readonly property var row: {
        service.host.revision;
        return service.host.model === null ? null : service.host.model.serviceRow;
    }

    title: "Service"
    visible: service.row !== null

    RowLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Layout.fillWidth: true
            text: service.row === null ? "" : service.row.text
            wrapMode: Text.Wrap
        }

        QQC2.Button {
            text: "Open Logs"
            visible: service.row !== null && service.row.opensLogs
            onClicked: service.host.model.openLogs()
        }

        QQC2.Button {
            text: service.row === null ? "" : service.row.button
            enabled: service.row !== null && service.row.canPress
            onClicked: service.host.model.pressService()
        }
    }
}
