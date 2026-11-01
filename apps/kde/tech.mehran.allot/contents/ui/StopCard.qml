import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView
import "../code/widget.mjs" as Allot

// Stop Service's question while jobs run or wait. It sits in the popup because a window of its own
// would neither stack nor take focus reliably from inside plasmashell.
Item {
    id: block

    required property string question
    required property var widget

    readonly property int margin: Kirigami.Units.smallSpacing

    function focusCancel() {
        cancel.forceActiveFocus(Qt.TabFocusReason);
    }

    implicitHeight: question === "" ? 0 : card.implicitHeight + margin * 1.5
    visible: question !== ""

    Rectangle {
        id: card

        x: block.margin
        y: block.margin * 1.5
        width: block.width - block.margin * 2
        implicitHeight: content.implicitHeight + Kirigami.Units.largeSpacing * 3
        radius: Kirigami.Units.smallSpacing * 1.5
        color: CardView.withAlpha(Kirigami.Theme.textColor, 0.04)
        border.width: 1
        border.color: CardView.edgeColor(Kirigami.Theme.textColor, Kirigami.Theme.backgroundColor,
            0.14)

        ColumnLayout {
            id: content

            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing * 1.5
            spacing: Kirigami.Units.largeSpacing

            PlasmaComponents.Label {
                Layout.fillWidth: true
                text: Allot.STOP_CONFIRM_TITLE
                font.weight: Font.DemiBold
            }

            PlasmaComponents.Label {
                Layout.fillWidth: true
                text: block.question
                wrapMode: Text.Wrap
            }

            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: Kirigami.Units.smallSpacing * 2

                PlasmaComponents.Button {
                    id: cancel

                    text: "Cancel"
                    Accessible.description: block.question
                    onClicked: block.widget.cancelStop()
                }

                PlasmaComponents.Button {
                    text: "Stop Service"
                    onClicked: block.widget.confirmStop()
                }
            }
        }
    }
}
