import QtQuick
import org.kde.kirigami as Kirigami

import "../code/card-view.mjs" as CardView

// The handoff's bullet bar: a track whose last part is hatched headroom, the used fill, a lighter
// part at the fill's end for GPU memory on shared RAM, and the reserved tick. The fills are
// rectangles with the bar's radius, as wide as the bar, of which clips show only the wanted part.
Item {
    id: bar

    required property var row
    required property bool animate

    readonly property int barHeight: 6
    readonly property int barRadius: 2
    readonly property int glideMs: CardView.scaledMs(CardView.GLIDE_MS, Kirigami.Units.longDuration)
    readonly property color fillColor: Kirigami.Theme.highlightColor
    property real used: row.used
    property real shared: row.shared === null ? 0 : row.shared

    implicitHeight: 14

    Behavior on used {
        enabled: bar.animate

        NumberAnimation {
            duration: bar.glideMs
            easing.type: Easing.OutQuad
        }
    }

    Behavior on shared {
        enabled: bar.animate

        NumberAnimation {
            duration: bar.glideMs
            easing.type: Easing.OutQuad
        }
    }

    HatchedTrack {
        y: (bar.height - bar.barHeight) / 2
        width: bar.width
        height: bar.barHeight
        radius: bar.barRadius
        headroom: bar.row.headroom
    }

    Item {
        y: (bar.height - bar.barHeight) / 2
        width: bar.width * bar.used
        height: bar.barHeight
        clip: true

        Rectangle {
            width: bar.width
            height: bar.barHeight
            radius: bar.barRadius
            color: bar.fillColor
        }
    }

    Item {
        x: bar.width * (bar.used - bar.shared)
        y: (bar.height - bar.barHeight) / 2
        width: bar.width * bar.shared
        height: bar.barHeight
        clip: true
        visible: bar.shared > 0

        Rectangle {
            x: -parent.x
            width: bar.width
            height: bar.barHeight
            radius: bar.barRadius
            color: Qt.lighter(bar.fillColor, 1.5)
        }
    }

    Rectangle {
        x: Math.round(bar.width * (bar.row.reserved === null ? 0 : bar.row.reserved)) - width / 2
        anchors.verticalCenter: parent.verticalCenter
        visible: bar.row.reserved !== null
        width: 4
        height: bar.height
        radius: 1
        color: Kirigami.Theme.backgroundColor

        Rectangle {
            anchors.centerIn: parent
            width: 2
            height: parent.height - 2
            color: Kirigami.Theme.textColor
        }
    }
}
