import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView
import "../code/widget.mjs" as Allot

// The first-run card. There is no Launch at login switch: plasmashell starts the widget and
// `allot install` enables allotd's unit.
Rectangle {
    id: card

    required property bool doNotDisturb
    required property bool notificationsOff
    required property var widget

    implicitHeight: content.implicitHeight + Kirigami.Units.largeSpacing * 3
    radius: Kirigami.Units.smallSpacing * 1.5
    color: CardView.withAlpha(Kirigami.Theme.textColor, 0.04)
    border.width: 1
    border.color: CardView.edgeColor(Kirigami.Theme.textColor, Kirigami.Theme.backgroundColor, 0.14)

    ColumnLayout {
        id: content

        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * 1.5
        spacing: Kirigami.Units.largeSpacing

        PlasmaComponents.Label {
            Layout.fillWidth: true
            text: Allot.WELCOME_TITLE
            font.weight: Font.DemiBold
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            text: Allot.WELCOME_BODY
            wrapMode: Text.Wrap
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            visible: card.doNotDisturb
            text: Allot.DO_NOT_DISTURB_TEXT
            font: Kirigami.Theme.smallFont
            opacity: 0.8
            wrapMode: Text.Wrap
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            visible: card.notificationsOff
            text: Allot.NOTIFICATIONS_OFF_CARD_TEXT
            font: Kirigami.Theme.smallFont
            opacity: 0.8
            wrapMode: Text.Wrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing * 2

            PlasmaComponents.Button {
                visible: card.notificationsOff
                text: "Open Notification Settings"
                flat: true
                onClicked: card.widget.openNotificationSettings()
            }

            Item {
                Layout.fillWidth: true
            }

            PlasmaComponents.Button {
                text: "Done"
                onClicked: card.widget.finishFirstRun()
            }
        }
    }
}
