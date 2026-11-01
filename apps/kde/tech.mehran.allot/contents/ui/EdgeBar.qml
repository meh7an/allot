import QtQuick
import org.kde.kirigami as Kirigami

import "../code/card-view.mjs" as CardView

// The 2 px bar along a card's bottom. Its track and fill are rectangles with the card's radius, as
// wide as the card, of which clips show only the bottom 2 px, so they follow the card's corners.
Item {
    id: bar

    required property string tone
    required property real fraction
    required property int radius
    required property bool animate

    readonly property color fillColor: {
        switch (tone) {
        case "running":
            return Kirigami.Theme.positiveTextColor;
        case "frozen":
            return Kirigami.Theme.disabledTextColor;
        default:
            return Kirigami.Theme.neutralTextColor;
        }
    }
    readonly property bool isDashed: tone === "waiting-dashed"
    property real share: fraction

    height: 2
    visible: tone !== "none"
    clip: true

    Behavior on share {
        enabled: bar.animate && bar.tone !== "frozen"

        NumberAnimation {
            duration: CardView.scaledMs(CardView.BAR_MS, Kirigami.Units.longDuration)
            easing.type: Easing.Linear
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: bar.radius * 2
        radius: bar.radius
        color: CardView.withAlpha(Kirigami.Theme.textColor, 0.08)
    }

    Item {
        anchors.bottom: parent.bottom
        width: parent.width * bar.share
        height: parent.height
        visible: !bar.isDashed
        clip: true

        Rectangle {
            anchors.bottom: parent.bottom
            width: bar.width
            height: bar.radius * 2
            radius: bar.radius
            color: bar.fillColor
        }
    }

    // Dashes have no curve to follow, so they stop short of the corners.
    Row {
        anchors.left: parent.left
        anchors.leftMargin: bar.radius
        anchors.right: parent.right
        anchors.rightMargin: bar.radius
        height: parent.height
        spacing: Kirigami.Units.smallSpacing * 1.5
        visible: bar.isDashed
        clip: true

        Repeater {
            model: bar.isDashed ? Math.ceil(bar.width / (Kirigami.Units.smallSpacing * 3)) : 0

            Rectangle {
                width: Kirigami.Units.smallSpacing * 1.5
                height: bar.height
                color: bar.fillColor
            }
        }
    }
}
