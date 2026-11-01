import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView

// The Recently finished heading and its card of rows.
ColumnLayout {
    id: recent

    required property var lines

    spacing: Kirigami.Units.smallSpacing * 1.5

    GroupHeading {
        Layout.fillWidth: true
        icon: CardView.CHECK_ICON
        iconColor: Kirigami.Theme.positiveTextColor
        text: "Recently finished"
    }

    Rectangle {
        Layout.fillWidth: true
        implicitHeight: rows.implicitHeight + border.width * 2
        radius: Kirigami.Units.smallSpacing * 1.5
        color: CardView.withAlpha(Kirigami.Theme.textColor, 0.04)
        border.width: 1
        border.color: CardView.withAlpha(Kirigami.Theme.textColor, 0.14)

        ColumnLayout {
            id: rows

            x: parent.border.width
            y: parent.border.width
            width: parent.width - parent.border.width * 2
            spacing: 0

            Repeater {
                model: recent.lines

                RowLayout {
                    id: line

                    required property var modelData

                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing + Kirigami.Units.smallSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing + Kirigami.Units.smallSpacing
                    Layout.minimumHeight: Kirigami.Units.iconSizes.medium + Kirigami.Units.smallSpacing
                    spacing: Kirigami.Units.largeSpacing

                    PlasmaComponents.Label {
                        Layout.fillWidth: true
                        text: line.modelData.command
                        font: Kirigami.Theme.fixedWidthFont
                        elide: Text.ElideRight
                        PlasmaComponents.ToolTip.text: line.modelData.command
                        PlasmaComponents.ToolTip.visible: commandHover.hovered

                        HoverHandler {
                            id: commandHover
                        }
                    }

                    PlasmaComponents.Label {
                        visible: text !== ""
                        text: line.modelData.shortId
                        font.family: Kirigami.Theme.fixedWidthFont.family
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        opacity: 0.8
                    }

                    PlasmaComponents.Label {
                        text: line.modelData.meta
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        font.features: { "tnum": 1 }
                        opacity: 0.8
                    }
                }
            }
        }
    }
}
