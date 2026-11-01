import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView

// One drive: label, usage bar with the read and write rates, and space. The tooltip is its mount
// point, since the label is only the last part.
RowLayout {
    id: drive

    required property var row
    required property bool animate

    readonly property int barHeight: 4
    readonly property int barRadius: 2
    property real used: row.used

    Layout.preferredHeight: Math.max(Kirigami.Units.largeSpacing * 3.5,
        middle.implicitHeight + Kirigami.Units.smallSpacing)
    spacing: Kirigami.Units.largeSpacing

    PlasmaComponents.ToolTip.text: row.id
    PlasmaComponents.ToolTip.visible: hover.hovered && row.id !== ""
    PlasmaComponents.ToolTip.delay: Kirigami.Units.toolTipDelay

    Behavior on used {
        enabled: drive.animate

        NumberAnimation {
            duration: CardView.scaledMs(CardView.GLIDE_MS, Kirigami.Units.longDuration)
            easing.type: Easing.OutQuad
        }
    }

    HoverHandler {
        id: hover
    }

    PlasmaComponents.Label {
        Layout.preferredWidth: Kirigami.Units.largeSpacing * 6
        text: drive.row.label
        font.pointSize: Kirigami.Theme.smallFont.pointSize
        font.weight: Font.DemiBold
        elide: Text.ElideMiddle
    }

    ColumnLayout {
        id: middle

        Layout.fillWidth: true
        Layout.alignment: Qt.AlignVCenter
        spacing: 2

        Rectangle {
            id: track

            Layout.fillWidth: true
            implicitHeight: drive.barHeight
            radius: drive.barRadius
            color: CardView.withAlpha(Kirigami.Theme.textColor, 0.08)

            Item {
                width: parent.width * drive.used
                height: parent.height
                clip: true

                Rectangle {
                    width: track.width
                    height: track.height
                    radius: drive.barRadius
                    color: CardView.withAlpha(Kirigami.Theme.textColor, 0.55)
                }
            }
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            visible: text !== ""
            text: drive.row.activity
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.features: { "tnum": 1 }
            opacity: 0.8
            elide: Text.ElideRight
        }
    }

    PlasmaComponents.Label {
        Layout.preferredWidth: Kirigami.Units.largeSpacing * 13.5
        horizontalAlignment: Text.AlignRight
        text: drive.row.value
        font.pointSize: Kirigami.Theme.smallFont.pointSize
        font.features: { "tnum": 1 }
        elide: Text.ElideLeft
    }
}
