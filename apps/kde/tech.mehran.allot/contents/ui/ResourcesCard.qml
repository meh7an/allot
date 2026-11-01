import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView

// The docked Resources card: a header that folds the rows open and shut, and the rows themselves.
// The rows stay laid out inside a clip while it folds, so nothing squeezes on the way.
Item {
    id: dock

    required property var view
    required property bool animate
    required property var widget

    readonly property int margin: Kirigami.Units.smallSpacing
    readonly property int rowPadding: Kirigami.Units.largeSpacing * 1.5
    readonly property string meterIds: CardView.rowIds(view.meters)
    readonly property string driveIds: CardView.rowIds(view.drives)

    implicitHeight: view.isVisible ? card.height + margin * 1.5 : 0
    visible: view.isVisible

    Rectangle {
        id: card

        x: dock.margin
        y: dock.margin * 1.5
        width: dock.width - dock.margin * 2
        height: header.height + fold.height + border.width * 2
        radius: Kirigami.Units.smallSpacing * 1.5
        color: CardView.withAlpha(Kirigami.Theme.textColor, 0.04)
        border.width: 1
        border.color: CardView.withAlpha(Kirigami.Theme.textColor, 0.14)

        QQC2.AbstractButton {
            id: header

            x: card.border.width
            y: card.border.width
            width: card.width - card.border.width * 2
            height: Kirigami.Units.largeSpacing * 5
            hoverEnabled: true
            focusPolicy: Qt.StrongFocus
            Accessible.name: dock.view.isExpanded ? "Collapse resources" : "Expand resources"
            onClicked: dock.widget.toggleResources()
            Keys.onReturnPressed: clicked()
            Keys.onEnterPressed: clicked()

            background: Rectangle {
                radius: Kirigami.Units.smallSpacing
                color: header.hovered ? CardView.withAlpha(Kirigami.Theme.textColor, 0.06)
                    : "transparent"
                border.width: header.visualFocus ? 1 : 0
                border.color: Kirigami.Theme.focusColor
            }

            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing

                PlasmaComponents.Label {
                    Layout.leftMargin: dock.rowPadding
                    text: "Resources"
                    font.weight: Font.DemiBold
                }

                PlasmaComponents.Label {
                    id: summary

                    // The text stays while it fades out, since a collapsed card's is what fades.
                    property string shownText: ""

                    Layout.fillWidth: true
                    text: shownText
                    font: Kirigami.Theme.smallFont
                    elide: Text.ElideRight
                    opacity: dock.view.isExpanded ? 0 : 0.8

                    Binding on shownText {
                        when: dock.view.summary !== ""
                        value: dock.view.summary
                        restoreMode: Binding.RestoreNone
                    }

                    Behavior on opacity {
                        enabled: dock.animate

                        NumberAnimation {
                            duration: CardView.scaledMs(CardView.SUMMARY_MS,
                                Kirigami.Units.longDuration)
                            easing.type: Easing.OutQuad
                        }
                    }
                }

                Kirigami.Icon {
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                    source: dock.view.isExpanded ? CardView.FOLD_DOWN_ICON : CardView.FOLD_UP_ICON
                }
            }
        }

        Item {
            id: fold

            x: card.border.width
            y: header.y + header.height
            width: header.width
            height: dock.view.isExpanded ? body.implicitHeight : 0
            clip: true

            Behavior on height {
                enabled: dock.animate

                NumberAnimation {
                    duration: CardView.scaledMs(CardView.FOLD_MS, Kirigami.Units.longDuration)
                    easing.type: Easing.OutQuad
                }
            }

            ColumnLayout {
                id: body

                x: dock.rowPadding
                width: fold.width - dock.rowPadding * 2
                spacing: 0

                Repeater {
                    model: CardView.splitIds(dock.meterIds)

                    MeterRow {
                        required property string modelData

                        Layout.fillWidth: true
                        row: CardView.rowById(dock.view.meters, modelData)
                        animate: dock.animate
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.smallSpacing
                    Layout.bottomMargin: Kirigami.Units.smallSpacing
                    visible: dock.view.showsSeparator
                    implicitHeight: 1
                    color: CardView.withAlpha(Kirigami.Theme.textColor, 0.14)
                }

                Repeater {
                    model: CardView.splitIds(dock.driveIds)

                    DriveRow {
                        required property string modelData

                        Layout.fillWidth: true
                        row: CardView.rowById(dock.view.drives, modelData)
                        animate: dock.animate
                    }
                }

                PlasmaComponents.Label {
                    Layout.fillWidth: true
                    Layout.bottomMargin: Kirigami.Units.largeSpacing
                    visible: dock.view.allOff
                    text: CardView.ALL_OFF_TEXT
                    font: Kirigami.Theme.smallFont
                    wrapMode: Text.Wrap
                }

                RowLayout {
                    Layout.topMargin: Kirigami.Units.smallSpacing
                    Layout.bottomMargin: Kirigami.Units.largeSpacing
                    visible: dock.view.showsLegend
                    spacing: Kirigami.Units.largeSpacing * 1.5

                    LegendEntry {
                        text: "Used"
                        swatchColor: Kirigami.Theme.highlightColor
                    }

                    LegendEntry {
                        visible: dock.view.showsSharedLegend
                        text: "GPU"
                        swatchColor: Qt.lighter(Kirigami.Theme.highlightColor, 1.5)
                    }

                    LegendEntry {
                        text: "Reserved"
                        swatchColor: Kirigami.Theme.textColor
                        swatchWidth: 2
                    }

                    LegendEntry {
                        text: "Headroom"
                        hatched: true
                    }
                }
            }
        }
    }
}
