import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView
import "../code/widget.mjs" as Allot

// One row of the popup's list. It enters and leaves by height and opacity, its top gap included,
// and asks the model to drop it once it has collapsed. Recently finished and the empty line enter
// after the row they replace has left.
Item {
    id: listRow

    required property string key
    required property string kind
    required property bool leaving
    required property bool entering
    required property var row
    required property bool animate
    required property var widget

    readonly property int gap: Kirigami.Units.smallSpacing * 1.5
    readonly property int margin: Kirigami.Units.smallSpacing
    readonly property int duration: CardView.scaledMs(CardView.ROW_MS, Kirigami.Units.longDuration)
    readonly property bool entersLate: entering && (kind === "recent" || kind === "nothing")
    property bool arrived: !entering
    property real shown: arrived && !leaving ? 1 : 0
    readonly property bool dark: CardView.isDark(Kirigami.Theme.backgroundColor)

    function finishLeaving() {
        widget.rowGone(key);
    }

    function groupColor(kind) {
        switch (kind) {
        case "benchmark":
            return CardView.benchmarkColor(dark);
        case "running":
            return Kirigami.Theme.positiveTextColor;
        default:
            return Kirigami.Theme.neutralTextColor;
        }
    }

    height: (content.implicitHeight + gap) * shown
    opacity: shown
    clip: shown < 1

    // The model is mid-update when `leaving` changes, so a row with no height to collapse waits.
    onLeavingChanged: {
        if (leaving && shown === 0) {
            Qt.callLater(finishLeaving);
        }
    }
    onShownChanged: {
        if (leaving && shown === 0) {
            finishLeaving();
        }
    }
    Component.onCompleted: {
        if (entering && !entersLate) {
            arrived = true;
        }
    }

    Behavior on shown {
        NumberAnimation {
            duration: listRow.duration
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.4, 0, 0.2, 1, 1, 1]
        }
    }

    Timer {
        interval: Math.max(1, listRow.duration)
        running: listRow.entersLate && !listRow.arrived
        onTriggered: listRow.arrived = true
    }

    Loader {
        id: content

        x: listRow.margin
        y: listRow.gap
        width: parent.width - listRow.margin * 2
        sourceComponent: {
            switch (listRow.kind) {
            case "heading":
                return headingRow;
            case "job":
                return jobRow;
            case "recent":
                return recentRow;
            default:
                return lineRow;
            }
        }
    }

    Component {
        id: headingRow

        GroupHeading {
            icon: Allot.groupIcon(listRow.row.group.kind)
            iconColor: listRow.groupColor(listRow.row.group.kind)
            text: listRow.row.group.heading
        }
    }

    Component {
        id: jobRow

        JobRow {
            job: listRow.row.job
            appIcon: listRow.row.icon
            animate: listRow.animate
            widget: listRow.widget
        }
    }

    Component {
        id: recentRow

        RecentCard {
            lines: listRow.row.lines
        }
    }

    Component {
        id: lineRow

        PlasmaComponents.Label {
            text: listRow.row.text
            font: Kirigami.Theme.smallFont
            elide: Text.ElideRight
            leftPadding: listRow.margin
            rightPadding: listRow.margin
        }
    }
}
