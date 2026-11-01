import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView

// The control at a card's right. One button shows Kill, Kill? or Remove, so a countdown or a tick
// never takes the focus off Kill?. All three stay laid out at its right edge and only the shown
// one is opaque; the button's width moves between theirs, so Kill? grows from Kill and the chip
// slides. It has no width without an action, from the moment a stop is sent.
QQC2.AbstractButton {
    id: button

    required property string shownAction
    required property string actionName
    required property bool animate

    readonly property int side: Kirigami.Units.iconSizes.medium
    readonly property int shownWidth: {
        switch (shownAction) {
        case "kill":
            return side;
        case "confirm":
            return confirm.implicitWidth + Kirigami.Units.largeSpacing * 2;
        case "remove":
            return remove.implicitWidth + Kirigami.Units.largeSpacing * 2;
        default:
            return 0;
        }
    }

    signal focusLeftConfirm()
    signal escapedConfirm()

    Layout.preferredWidth: shownWidth
    Layout.preferredHeight: side
    implicitWidth: shownWidth
    implicitHeight: side
    enabled: shownAction !== "none"
    focusPolicy: Qt.StrongFocus
    hoverEnabled: true
    clip: true
    padding: 0
    Accessible.name: actionName
    PlasmaComponents.ToolTip.text: actionName
    PlasmaComponents.ToolTip.visible: hovered && shownAction !== "confirm"
    PlasmaComponents.ToolTip.delay: Kirigami.Units.toolTipDelay

    // The handoff moves focus to the confirm, so Enter or Space can give the second press.
    onShownActionChanged: {
        if (shownAction === "confirm") {
            forceActiveFocus();
        }
    }
    onActiveFocusChanged: {
        if (!activeFocus && shownAction === "confirm") {
            focusLeftConfirm();
        }
    }
    Keys.onReturnPressed: clicked()
    Keys.onEnterPressed: clicked()
    Keys.onEscapePressed: (event) => {
        if (shownAction === "confirm") {
            escapedConfirm();
        } else {
            event.accepted = false;
        }
    }

    Behavior on Layout.preferredWidth {
        enabled: button.animate

        NumberAnimation {
            duration: CardView.scaledMs(button.shownAction === "confirm" ? CardView.ACTION_GROW_MS
                : CardView.ACTION_SHRINK_MS, Kirigami.Units.longDuration)
            easing.type: Easing.OutCubic
        }
    }

    background: Rectangle {
        radius: Kirigami.Units.smallSpacing
        color: {
            switch (button.shownAction) {
            case "confirm":
                return button.hovered ? Qt.darker(Kirigami.Theme.negativeTextColor, 1.2)
                    : Kirigami.Theme.negativeTextColor;
            case "kill":
                return button.hovered ? CardView.withAlpha(Kirigami.Theme.negativeTextColor, 0.14)
                    : "transparent";
            case "remove":
                return button.hovered ? CardView.withAlpha(Kirigami.Theme.textColor, 0.1)
                    : "transparent";
            default:
                return "transparent";
            }
        }
        border.width: button.visualFocus ? 1 : 0
        border.color: Kirigami.Theme.focusColor
    }

    contentItem: Item {
        Kirigami.Icon {
            anchors.right: parent.right
            anchors.rightMargin: (button.side - width) / 2
            anchors.verticalCenter: parent.verticalCenter
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            opacity: button.shownAction === "kill" ? 0.8 : 0
            source: CardView.KILL_ICON
            isMask: true
            color: Kirigami.Theme.textColor
        }

        RowLayout {
            id: confirm

            anchors.right: parent.right
            anchors.rightMargin: Kirigami.Units.largeSpacing
            anchors.verticalCenter: parent.verticalCenter
            spacing: Kirigami.Units.smallSpacing
            opacity: button.shownAction === "confirm" ? 1 : 0

            Kirigami.Icon {
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                source: CardView.KILL_ICON
                isMask: true
                color: "white"
            }

            PlasmaComponents.Label {
                text: "Kill?"
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                font.weight: Font.DemiBold
                color: "white"
            }
        }

        PlasmaComponents.Label {
            id: remove

            anchors.right: parent.right
            anchors.rightMargin: Kirigami.Units.largeSpacing
            anchors.verticalCenter: parent.verticalCenter
            opacity: button.shownAction === "remove" ? 0.8 : 0
            text: "Remove"
            font: Kirigami.Theme.smallFont
        }
    }
}
