import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView

// One job's card: the session strip, the job row, the stepper and the edge bar.
Rectangle {
    id: cardView

    required property var job
    required property var appIcon
    required property bool animate
    property var widget: null

    readonly property var card: job.card
    readonly property bool dark: CardView.isDark(Kirigami.Theme.backgroundColor)
    readonly property color benchmarkColor: CardView.benchmarkColor(dark)
    readonly property real endedOpacity: card.isEnded ? CardView.ENDED_OPACITY : 1
    readonly property int tileSize: Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing
    readonly property int rowGap: Kirigami.Units.largeSpacing
    readonly property int rowLeft: Kirigami.Units.largeSpacing + Kirigami.Units.smallSpacing
    readonly property int innerRadius: radius - border.width

    function press() {
        switch (card.action) {
        case "kill":
        case "confirm":
            widget.pressKill(job.id);
            break;
        case "remove":
            widget.pressRemove(job.id);
            break;
        }
    }

    implicitHeight: body.implicitHeight + border.width * 2
    radius: Kirigami.Units.smallSpacing * 1.5
    color: CardView.withAlpha(Kirigami.Theme.textColor, 0.04)
    border.width: 1
    border.color: CardView.edgeColor(Kirigami.Theme.textColor, Kirigami.Theme.backgroundColor, 0.14)

    ColumnLayout {
        id: body

        x: cardView.border.width
        y: cardView.border.width
        width: parent.width - cardView.border.width * 2
        spacing: 0

        Item {
            Layout.fillWidth: true
            implicitHeight: stripLine.implicitHeight + Kirigami.Units.smallSpacing * 2

            // Only the top of this taller rectangle shows, which rounds the strip's top corners.
            Item {
                anchors.fill: parent
                clip: true

                Rectangle {
                    width: parent.width
                    height: parent.height + cardView.radius * 2
                    radius: cardView.innerRadius
                    color: CardView.withAlpha(Kirigami.Theme.textColor, 0.07)
                }
            }

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: cardView.border.color
            }

            RowLayout {
                id: stripLine

                anchors.verticalCenter: parent.verticalCenter
                x: cardView.rowLeft
                width: parent.width - cardView.rowLeft * 2
                spacing: cardView.rowGap

                PlasmaComponents.Label {
                    text: cardView.job.session.shortId
                    font.family: Kirigami.Theme.fixedWidthFont.family
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                }

                PlasmaComponents.Label {
                    Layout.fillWidth: true
                    text: cardView.job.session.project
                    font: Kirigami.Theme.smallFont
                    elide: Text.ElideRight
                    opacity: 0.8
                    PlasmaComponents.ToolTip.text: cardView.job.session.projectPath
                    PlasmaComponents.ToolTip.visible: projectHover.hovered
                        && cardView.job.session.projectPath !== ""

                    HoverHandler {
                        id: projectHover
                    }
                }

                PlasmaComponents.Label {
                    visible: text !== ""
                    text: cardView.job.session.usage
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    font.features: { "tnum": 1 }
                    opacity: 0.8
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.smallSpacing
            Layout.leftMargin: cardView.rowLeft
            Layout.rightMargin: Kirigami.Units.smallSpacing
            Layout.minimumHeight: Kirigami.Units.iconSizes.medium
            spacing: cardView.rowGap

            Rectangle {
                implicitWidth: cardView.tileSize
                implicitHeight: cardView.tileSize
                radius: Kirigami.Units.smallSpacing
                opacity: cardView.endedOpacity
                color: cardView.card.isQueued ? "transparent"
                    : CardView.withAlpha(Kirigami.Theme.textColor, 0.07)
                border.width: cardView.card.isQueued ? 1 : 0
                border.color: CardView.edgeColor(Kirigami.Theme.textColor,
                    Kirigami.Theme.backgroundColor, 0.3)

                Kirigami.Icon {
                    anchors.centerIn: parent
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                    visible: cardView.appIcon !== null
                    source: cardView.appIcon ?? ""
                }

                PlasmaComponents.Label {
                    anchors.centerIn: parent
                    visible: cardView.appIcon === null
                    text: cardView.card.tile
                    font.family: Kirigami.Theme.fixedWidthFont.family
                    font.pointSize: Math.max(6, Kirigami.Theme.smallFont.pointSize - 1)
                    font.weight: Font.DemiBold
                    Accessible.ignored: true
                }
            }

            PlasmaComponents.Label {
                Layout.fillWidth: true
                text: cardView.job.command
                font.family: Kirigami.Theme.fixedWidthFont.family
                font.strikeout: cardView.card.isStruck
                elide: Text.ElideRight
                opacity: cardView.endedOpacity
                Accessible.name: cardView.job.command
                PlasmaComponents.ToolTip.text: cardView.job.command
                PlasmaComponents.ToolTip.visible: commandHover.hovered

                HoverHandler {
                    id: commandHover
                }
            }

            Rectangle {
                implicitWidth: chipLine.implicitWidth + Kirigami.Units.largeSpacing * 2
                implicitHeight: Math.max(Kirigami.Units.iconSizes.small,
                    chipLine.implicitHeight) + Kirigami.Units.smallSpacing / 2
                radius: height / 2
                opacity: cardView.endedOpacity
                color: cardView.card.isBenchmark ? cardView.benchmarkColor
                    : CardView.withAlpha(Kirigami.Theme.textColor, 0.09)
                PlasmaComponents.ToolTip.text: cardView.card.chipTooltip
                PlasmaComponents.ToolTip.visible: chipHover.hovered

                HoverHandler {
                    id: chipHover
                }

                RowLayout {
                    id: chipLine

                    readonly property color ink: cardView.card.isBenchmark
                        ? CardView.benchmarkInk(cardView.dark) : Kirigami.Theme.textColor

                    anchors.centerIn: parent
                    spacing: Kirigami.Units.smallSpacing

                    Kirigami.Icon {
                        visible: cardView.card.isBenchmark
                        implicitWidth: Kirigami.Units.iconSizes.small - Kirigami.Units.smallSpacing
                        implicitHeight: implicitWidth
                        source: CardView.TIMER_ICON
                        isMask: true
                        color: chipLine.ink
                    }

                    PlasmaComponents.Label {
                        text: cardView.card.chip
                        font: Kirigami.Theme.smallFont
                        color: chipLine.ink
                    }
                }
            }

            CardAction {
                shownAction: cardView.card.action
                actionName: cardView.card.actionName
                animate: cardView.animate
                describe: () => CardView.cardDescription(cardView.job)
                onClicked: cardView.press()
                onFocusLeftConfirm: cardView.widget.cancelConfirm(cardView.job.id)
                onEscapedConfirm: cardView.widget.cancelConfirms()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: cardView.rowLeft + cardView.tileSize + cardView.rowGap
            Layout.rightMargin: cardView.rowGap
            spacing: cardView.rowGap

            Kirigami.Icon {
                Layout.alignment: Qt.AlignTop
                visible: cardView.card.icon !== "none"
                implicitWidth: Kirigami.Units.iconSizes.small - Kirigami.Units.smallSpacing / 2
                implicitHeight: implicitWidth
                source: cardView.card.icon === "check" ? CardView.CHECK_ICON : CardView.TIMER_ICON
                isMask: true
                color: cardView.card.icon === "check" ? Kirigami.Theme.positiveTextColor
                    : cardView.benchmarkColor
            }

            PlasmaComponents.Label {
                Layout.fillWidth: true
                text: CardView.detailMarkup(cardView.card)
                textFormat: Text.StyledText
                font: Kirigami.Theme.smallFont
                wrapMode: CardView.detailWraps(cardView.card) ? Text.Wrap : Text.NoWrap
                elide: CardView.detailWraps(cardView.card) ? Text.ElideNone : Text.ElideRight
                color: cardView.card.detailIsDanger ? Kirigami.Theme.negativeTextColor
                    : Kirigami.Theme.textColor
                opacity: cardView.card.detailIsDanger || cardView.card.strong !== "" ? 1 : 0.8
                PlasmaComponents.ToolTip.text: cardView.card.detailTooltip
                PlasmaComponents.ToolTip.visible: detailHover.hovered
                    && cardView.card.detailTooltip !== ""

                HoverHandler {
                    id: detailHover
                }
            }

            PlasmaComponents.Label {
                Layout.alignment: Qt.AlignTop
                visible: text !== ""
                text: cardView.card.time
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                font.features: { "tnum": 1 }
                color: cardView.card.timeIsWarning ? Kirigami.Theme.neutralTextColor
                    : Kirigami.Theme.textColor
            }
        }

        Stepper {
            Layout.fillWidth: true
            Layout.topMargin: cardView.rowGap
            Layout.leftMargin: cardView.rowLeft + cardView.tileSize + cardView.rowGap
            Layout.rightMargin: cardView.rowGap
            visible: cardView.card.steps !== null
            steps: cardView.card.steps
            color: cardView.benchmarkColor
            animate: cardView.animate
        }

        Item {
            implicitHeight: Kirigami.Units.smallSpacing * 2.5
        }
    }

    EdgeBar {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: cardView.border.width
        tone: cardView.card.barTone
        fraction: cardView.card.barFraction
        radius: cardView.innerRadius
        animate: cardView.animate
    }
}
