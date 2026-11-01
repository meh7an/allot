import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView

// The red banner and the caption above the last known cards. It stays undimmed, so it lives above
// the list, and folds with the rows' motion when the service stops or comes back.
Item {
    id: block

    required property var banner
    required property bool animate
    required property var widget

    readonly property int margin: Kirigami.Units.smallSpacing
    // The text stays while the block folds away.
    property var shownBanner: ({ text: "", problem: "", button: "none", caption: "" })
    property real shown: banner === null ? 0 : 1

    onBannerChanged: {
        if (banner !== null) {
            shownBanner = banner;
        }
    }

    implicitHeight: (content.implicitHeight + margin * 1.5) * shown
    opacity: shown
    clip: shown < 1
    visible: shown > 0

    Behavior on shown {
        enabled: block.animate

        NumberAnimation {
            duration: CardView.scaledMs(CardView.ROW_MS, Kirigami.Units.longDuration)
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.4, 0, 0.2, 1, 1, 1]
        }
    }

    ColumnLayout {
        id: content

        x: block.margin
        y: block.margin * 1.5
        width: block.width - block.margin * 2
        spacing: block.margin

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: bannerRow.implicitHeight + Kirigami.Units.smallSpacing * 3
            radius: Kirigami.Units.smallSpacing * 1.5
            color: CardView.withAlpha(Kirigami.Theme.negativeTextColor, 0.08)
            border.width: 1
            border.color: CardView.withAlpha(Kirigami.Theme.negativeTextColor, 0.3)

            RowLayout {
                id: bannerRow

                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: Kirigami.Units.largeSpacing
                anchors.rightMargin: Kirigami.Units.smallSpacing * 1.5
                spacing: Kirigami.Units.largeSpacing

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0

                    PlasmaComponents.Label {
                        Layout.fillWidth: true
                        text: block.shownBanner.text
                        color: Kirigami.Theme.negativeTextColor
                        font.weight: Font.DemiBold
                        wrapMode: Text.Wrap
                    }

                    PlasmaComponents.Label {
                        Layout.fillWidth: true
                        visible: text !== ""
                        text: block.shownBanner.problem
                        color: Kirigami.Theme.negativeTextColor
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        wrapMode: Text.Wrap
                    }
                }

                PlasmaComponents.Button {
                    visible: block.shownBanner.button !== "none"
                    enabled: block.shownBanner.button === "start"
                    text: block.shownBanner.button === "starting" ? "Starting…" : "Start Service"
                    onClicked: block.widget.startService()
                }
            }
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.smallSpacing
            visible: text !== ""
            text: block.shownBanner.caption
            font: Kirigami.Theme.smallFont
            opacity: 0.8
        }
    }
}
