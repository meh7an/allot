import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View
import "../code/widget.mjs" as Allot

// The Panel page: the widget's own meters and notifications in tray.json, which work in every
// allotd state and save at once. The popup follows at once: the window runs in the widget's
// process and changes the widget's own model.
QQC2.ScrollView {
    id: page

    property var host: null
    readonly property var view: View.panelView(page.host.panelSettings, page.host.sharesMemory)

    contentWidth: availableWidth

    ColumnLayout {
        width: page.availableWidth
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            level: 1
            text: "Panel"
        }

        SettingsSection {
            title: View.METERS_TITLE
            help: View.METERS_HELP

            Repeater {
                model: page.view.meters.length

                QQC2.CheckBox {
                    required property int index
                    readonly property var row: page.view.meters[index]

                    Layout.leftMargin: row.indented ? Kirigami.Units.gridUnit : 0
                    text: row.label
                    checked: row.checked
                    enabled: row.enabled
                    onToggled: page.host.widget.setPanelToggle(row.toggle, checked)
                }
            }
        }

        SettingsSection {
            title: View.NOTIFICATIONS_TITLE

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                type: Kirigami.MessageType.Information
                text: Allot.NOTIFICATIONS_OFF_TEXT
                visible: page.host.notificationsOff
                actions: [
                    Kirigami.Action {
                        text: "Open Notification Settings…"
                        icon.name: "preferences-desktop-notification"
                        onTriggered: page.host.widget.openNotificationSettings()
                    }
                ]
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                type: Kirigami.MessageType.Information
                text: View.DO_NOT_DISTURB_TEXT
                visible: page.host.doNotDisturb
            }

            Repeater {
                model: page.view.notifications.length

                ColumnLayout {
                    required property int index
                    readonly property var row: page.view.notifications[index]

                    Layout.fillWidth: true
                    spacing: 0

                    QQC2.CheckBox {
                        id: box

                        text: parent.row.label
                        checked: parent.row.checked
                        enabled: parent.row.enabled
                        Accessible.description: parent.row.help
                        onToggled: page.host.widget.setPanelToggle(parent.row.toggle, checked)
                    }

                    HelpLabel {
                        Layout.leftMargin: box.indicator.width + box.spacing
                        text: View.escaped(parent.row.help)
                        visible: parent.row.help !== ""
                    }
                }
            }
        }

        QQC2.Button {
            Layout.margins: Kirigami.Units.largeSpacing
            text: "Reset Page…"
            enabled: page.view.canReset
            QQC2.ToolTip.text: View.NOTHING_TO_RESET
            QQC2.ToolTip.visible: hovered && !enabled
            onClicked: {
                const confirmation = View.panelResetConfirmation(page.host.panelSettings,
                    page.host.sharesMemory);
                if (confirmation !== null) {
                    page.host.confirm(confirmation, () => page.host.widget.resetPanel());
                }
            }
        }
    }
}
